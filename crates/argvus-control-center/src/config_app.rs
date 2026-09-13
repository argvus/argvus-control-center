use std::sync::Arc;

use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedOperation, PrivilegedRequest, SystemSettingsOperation},
  process::SystemProcessRunner,
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
    Self {
      lang,
      theme,
      icons: config.icons(),
      selected: 0,
      status: None,
      operation,
      jobs: JobManager::default(),
      save_job: None,
    }
  }

  pub fn rows(&self) -> Vec<String> {
    vec![format!(
      "[{}] {}",
      if self.icons { "✓" } else { " " },
      tr(self.lang, "Icones", "Icons")
    )]
  }

  pub fn breadcrumb(&self) -> String {
    tr(self.lang, "Configuração", "Configuration").into()
  }

  pub fn footer_hints(&self) -> &'static str {
    tr(
      self.lang,
      "↑/↓ Navegar   Enter/Space Alternar   ←/Esc Voltar   ? Ajuda",
      "↑/↓ Navigate   Enter/Space Toggle   ←/Esc Back   ? Help",
    )
  }

  pub fn handle(&mut self, key: KeyCode) -> bool {
    match key {
      KeyCode::Esc | KeyCode::Left => true,
      KeyCode::Enter | KeyCode::Char(' ') => {
        self.toggle();
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
      text: tr(
        self.lang,
        "Salvando configuração...",
        "Saving configuration...",
      )
      .into(),
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
            "Aplicado nesta sessão, mas não foi possível gravar a configuração",
            "Applied this session, but the configuration could not be saved"
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

  fn persist_failed(&mut self, text: String) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Warning,
      text,
    });
  }

  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.save_job
      && let JobState::Finished(result) = job.try_state()
    {
      self.save_job = None;
      match result {
        Ok(()) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: if self.icons {
              tr(self.lang, "Icones ativados", "Icons enabled").into()
            } else {
              tr(self.lang, "Icones desativados", "Icons disabled").into()
            },
          });
        }
        Err(error) => {
          self.persist_failed(format!(
            "{}: {error}",
            tr(
              self.lang,
              "Aplicado nesta sessão, mas não foi possível gravar a configuração",
              "Applied this session, but the configuration could not be saved"
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
        tr(self.lang, "Aparência", "Appearance")
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
      Lang::Pt,
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
    let mut app = ConfigApp::with_operation(Lang::Pt, Theme::load(), Arc::new(FailingOperation));
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
    let app = ConfigApp::new(Lang::Pt, Theme::load());
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
      Lang::Pt,
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
    let app = ConfigApp::new(Lang::Pt, Theme::load());
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
