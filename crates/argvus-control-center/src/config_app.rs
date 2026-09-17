use std::sync::Arc;
use std::time::{Duration, Instant};

use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedOperation, PrivilegedRequest, SystemSettingsOperation},
  process::{ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  components::{StatusKind, StatusMessage},
  page::{list, readonly, shell, status},
};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;

type ConfigOperation = Arc<dyn PrivilegedOperation>;

/// The ARGVUS Control Center configuration screen. Currently hosts the
/// option that enables/disables the decorative icons across the whole layout.
pub struct ConfigApp {
  lang: Lang,
  theme: Theme,
  pub icons: bool,
  selected: usize,
  pub status: Option<StatusMessage>,
  operation: ConfigOperation,
  jobs: JobManager,
  save_job: Option<JobHandle<()>>,
  dnd_enabled: bool,
  dnd_job: Option<JobHandle<(bool, bool)>>,
  dnd_last_refresh: Instant,
}

impl ConfigApp {
  pub fn new(lang: Lang, theme: Theme) -> Self {
    let executable = std::env::current_exe()
      .map(|path| path.to_string_lossy().into_owned())
      .unwrap_or_else(|_| "argvus-control-center".into());
    Self::with_operation(
      lang,
      theme,
      Arc::new(SystemSettingsOperation::new(
        SystemProcessRunner,
        executable,
      )),
    )
  }

  fn with_operation(lang: Lang, theme: Theme, operation: ConfigOperation) -> Self {
    let config = AppConfig::load();
    let mut app = Self {
      lang,
      theme,
      icons: config.icons(),
      selected: 0,
      status: None,
      operation,
      jobs: JobManager::default(),
      save_job: None,
      dnd_enabled: false,
      dnd_job: None,
      dnd_last_refresh: Instant::now() - Duration::from_secs(2),
    };
    app.refresh_dnd();
    app
  }

  pub fn rows(&self) -> Vec<String> {
    vec![
      format!(
        "[{}] {}",
        if self.icons { "✓" } else { " " },
        tr(self.lang, "control_center.icons")
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon(if self.dnd_enabled { "🔕" } else { "🔔" }),
        tr(self.lang, "control_center.do_not_disturb"),
        if self.dnd_enabled {
          tr(self.lang, "control_center.enabled")
        } else {
          tr(self.lang, "control_center.disabled")
        }
      ),
    ]
  }

  pub fn breadcrumb(&self) -> String {
    tr(self.lang, "control_center.configuration").into()
  }

  pub fn footer_hints(&self) -> &'static str {
    tr(
      self.lang,
      "control_center.navigate_enter_space_toggle_esc_back_help",
    )
  }

  pub fn handle(&mut self, key: KeyCode) -> bool {
    match key {
      KeyCode::Esc | KeyCode::Left => true,
      KeyCode::Enter | KeyCode::Char(' ') => {
        if self.selected == 0 {
          self.toggle();
        } else {
          self.toggle_dnd();
        }
        false
      }
      KeyCode::Up | KeyCode::Char('k') | KeyCode::Home | KeyCode::PageUp => {
        self.selected = self.selected.saturating_sub(1);
        self.normalize();
        false
      }
      KeyCode::Down | KeyCode::Char('j') | KeyCode::End | KeyCode::PageDown => {
        self.selected = self.selected.saturating_add(1);
        self.normalize();
        false
      }
      _ => false,
    }
  }

  fn normalize(&mut self) {
    self.selected = self.selected.min(self.rows().len().saturating_sub(1));
  }

  /// Applies the toggle in-session immediately and persists it through the
  /// privileged system-settings flow (polkit). Result is reported on poll().
  pub fn toggle(&mut self) {
    if self.save_job.is_some() {
      return;
    }
    self.icons = !self.icons;
    AppConfig::set_session_icons(self.icons);
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "control_center.saving_configuration").into(),
    });
    let operation = Arc::clone(&self.operation);
    let enabled = self.icons.to_string();
    let request = match PrivilegedRequest::new("config", "icons", vec![enabled]) {
      Ok(request) => request,
      Err(error) => {
        self.persist_failed(format!(
          "{}: {error}",
          tr(
            self.lang,
            "control_center.applied_this_session_but_the_configuration_could_not_be_saved"
          )
        ));
        return;
      }
    };
    self.save_job = Some(self.jobs.spawn(move |_| {
      let output = operation.execute(&request)?;
      if output.status == Some(0) {
        Ok(())
      } else {
        Err(terminal_text(
          String::from_utf8_lossy(&output.stderr).trim(),
        ))
      }
    }));
  }

  fn refresh_dnd(&mut self) {
    if self.dnd_job.is_some() {
      return;
    }
    self.dnd_job = Some(self.jobs.spawn(|_| {
      read_dnd_command(
        &ProcessRequest::new("argvus-notifications")
          .arg("dnd")
          .arg("status"),
      )
      .map(|enabled| (enabled, false))
    }));
    self.dnd_last_refresh = Instant::now();
  }

  fn toggle_dnd(&mut self) {
    if self.dnd_job.is_some() {
      return;
    }
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "control_center.applying").into(),
    });
    self.dnd_job = Some(self.jobs.spawn(move |_| {
      read_dnd_command(
        &ProcessRequest::new("argvus-notifications")
          .arg("dnd")
          .arg("toggle"),
      )
      .map(|state| (state, true))
    }));
  }

  fn persist_failed(&mut self, text: String) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Warning,
      text,
    });
  }

  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if self.dnd_job.is_none() && self.dnd_last_refresh.elapsed() >= Duration::from_secs(2) {
      self.refresh_dnd();
    }
    if let Some(job) = &self.dnd_job
      && let JobState::Finished(result) = job.try_state()
    {
      self.dnd_job = None;
      match result {
        Ok((enabled, action)) => {
          self.dnd_enabled = enabled;
          if action {
            self.status = Some(StatusMessage {
              kind: StatusKind::Success,
              text: if enabled {
                tr(self.lang, "control_center.dnd_enabled").into()
              } else {
                tr(self.lang, "control_center.dnd_disabled").into()
              },
            });
          } else {
            self.status = None;
          }
        }
        Err(error) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: format!(
              "{}: {error}",
              tr(self.lang, "control_center.notification_state_error")
            ),
          });
        }
      }
      changed = true;
    }
    if let Some(job) = &self.save_job
      && let JobState::Finished(result) = job.try_state()
    {
      self.save_job = None;
      match result {
        Ok(()) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: if self.icons {
              tr(self.lang, "control_center.icons_enabled").into()
            } else {
              tr(self.lang, "control_center.icons_disabled").into()
            },
          });
        }
        Err(error) => {
          self.persist_failed(format!(
            "{}: {error}",
            tr(
              self.lang,
              "control_center.applied_this_session_but_the_configuration_could_not_be_saved"
            )
          ));
        }
      }
      changed = true;
    }
    changed
  }

  pub fn draw(&self, frame: &mut Frame) {
    let area = frame.area();
    let body = shell(
      frame,
      area,
      &self.theme,
      &self.breadcrumb(),
      self.footer_hints(),
    );
    let split = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(body);
    readonly(
      frame,
      split[0],
      &self.theme,
      &[Line::from(format!(
        "  {}",
        tr(self.lang, "control_center.appearance")
      ))],
    );
    let rows = self.rows();
    list(
      frame,
      split[1],
      &self.theme,
      &rows,
      self.selected.min(rows.len().saturating_sub(1)),
    );
    if let Some(status_message) = &self.status {
      status(frame, body, &self.theme, status_message);
    }
  }
}

fn read_dnd_command(request: &ProcessRequest) -> Result<bool, String> {
  let output = SystemProcessRunner
    .run(&request.clone().timeout(Duration::from_secs(2)))
    .map_err(|error| error.to_string())?;
  if output.timed_out || output.status != Some(0) {
    let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    return Err(if error.is_empty() {
      "notification backend failed".into()
    } else {
      error
    });
  }
  match String::from_utf8_lossy(&output.stdout).trim() {
    "dnd=true" => Ok(true),
    "dnd=false" => Ok(false),
    value => Err(format!("invalid notification state: {value}")),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use argvus_control_center_core::process::ProcessOutput;
  use std::path::PathBuf;
  use std::sync::Mutex;
  use std::sync::atomic::AtomicUsize;
  use std::time::Duration;

  static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());
  static ENV_TEST_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

  #[derive(Default)]
  struct RecordingOperation(Mutex<Option<PrivilegedRequest>>);

  impl PrivilegedOperation for RecordingOperation {
    fn execute(&self, request: &PrivilegedRequest) -> Result<ProcessOutput, String> {
      *self.0.lock().unwrap() = Some(request.clone());
      Ok(ProcessOutput {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: Some(0),
        timed_out: false,
      })
    }
  }

  #[derive(Default)]
  struct FailingOperation;

  impl PrivilegedOperation for FailingOperation {
    fn execute(&self, _request: &PrivilegedRequest) -> Result<ProcessOutput, String> {
      Err("simulated denial".into())
    }
  }

  fn with_config(on_icons: bool) -> (PathBuf, PathBuf) {
    let sequence = ENV_TEST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
      "argvus-cc-config-app-{}-{sequence}",
      std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
      &path,
      if on_icons {
        "[Appearance]\nicons = true\n"
      } else {
        "[Appearance]\nicons = false\n"
      },
    )
    .unwrap();
    (dir, path)
  }

  fn drain_poll(app: &mut ConfigApp) {
    for _ in 0..200 {
      if app.poll() {
        return;
      }
      std::thread::sleep(Duration::from_millis(2));
    }
    panic!("config save job did not finish");
  }

  #[test]
  fn toggling_icons_flips_the_session_and_requests_a_privileged_save() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    let (dir, path) = with_config(true);
    unsafe { std::env::set_var("ARGVUS_CONFIG_PATH", &path) };
    let recorder = Arc::new(RecordingOperation::default());
    let mut app = ConfigApp::with_operation(
      Lang::for_locale("pt-BR"),
      Theme::load(),
      Arc::clone(&recorder) as ConfigOperation,
    );
    assert!(app.icons);
    app.toggle();
    assert!(!app.icons);
    assert!(!AppConfig::icons_enabled());
    drain_poll(&mut app);
    let requested = recorder.0.lock().unwrap().clone().unwrap();
    assert_eq!(requested.domain, "config");
    assert_eq!(requested.action, "icons");
    assert_eq!(requested.arguments, vec![String::from("false")]);
    assert!(matches!(
      app.status,
      Some(StatusMessage {
        kind: StatusKind::Success,
        ..
      })
    ));
    unsafe { std::env::remove_var("ARGVUS_CONFIG_PATH") };
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn failed_save_reports_applied_this_session_warning() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    let (dir, path) = with_config(true);
    unsafe { std::env::set_var("ARGVUS_CONFIG_PATH", &path) };
    let mut app = ConfigApp::with_operation(
      Lang::for_locale("pt-BR"),
      Theme::load(),
      Arc::new(FailingOperation),
    );
    app.toggle();
    assert!(!app.icons);
    drain_poll(&mut app);
    let Some(message) = app.status else {
      panic!("expected a status message");
    };
    assert_eq!(message.kind, StatusKind::Warning);
    assert!(message.text.contains("simulated denial"), "{message:?}?");
    unsafe { std::env::remove_var("ARGVUS_CONFIG_PATH") };
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn checkbox_label_tracks_icon_state() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    let (dir, path) = with_config(true);
    unsafe { std::env::set_var("ARGVUS_CONFIG_PATH", &path) };
    let app = ConfigApp::new(Lang::for_locale("pt-BR"), Theme::load());
    let row = app.rows().into_iter().next().unwrap();
    assert!(row.starts_with("[✓] Icones"), "{row}");
    unsafe { std::env::remove_var("ARGVUS_CONFIG_PATH") };
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn escape_leaves_and_enter_toggles() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    let (dir, path) = with_config(true);
    unsafe { std::env::set_var("ARGVUS_CONFIG_PATH", &path) };
    let mut app = ConfigApp::with_operation(
      Lang::for_locale("pt-BR"),
      Theme::load(),
      Arc::new(RecordingOperation::default()),
    );
    assert!(app.handle(KeyCode::Esc), "Esc must signal going back");
    assert!(!app.handle(KeyCode::Enter));
    assert!(!app.icons);
    unsafe { std::env::remove_var("ARGVUS_CONFIG_PATH") };
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn draws_the_checkbox_screen() {
    let _guard = ENV_TEST_LOCK.lock().unwrap();
    let (dir, path) = with_config(true);
    unsafe { std::env::set_var("ARGVUS_CONFIG_PATH", &path) };
    let app = ConfigApp::new(Lang::for_locale("pt-BR"), Theme::load());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(90, 24)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("ARGVUS"));
    assert!(text.contains("Configuração") || text.contains("Configuration"));
    assert!(text.contains("[✓]") || text.contains("[ ]"));
    assert!(text.contains("Icones") || text.contains("Icons"));
    unsafe { std::env::remove_var("ARGVUS_CONFIG_PATH") };
    let _ = std::fs::remove_dir_all(&dir);
  }
}
