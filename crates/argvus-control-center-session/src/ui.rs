use crate::{
  backend,
  journal::JournalEntry,
  model::{AutostartEntry, Component, ComponentStatus, DiagnosticsEntry, SessionPage, manifest},
};
use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::buttons::{Button, ButtonKind};
use argvus_tui::components::{
  ConfirmationDialog, ConfirmationOutcome, ConfirmationState, StatusKind, StatusMessage,
  draw_confirmation,
};
use argvus_tui::page::{list, readonly, shell, status};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;

#[derive(Debug, Clone)]
enum Pending {
  Restart(String),
  ToggleAutostart(String, bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionButton {
  Restart,
  Enable,
  Disable,
  NextLogFilter,
  Refresh,
}

#[derive(Debug, Clone)]
enum JobData {
  HomeSummary(Vec<Component>, Vec<AutostartEntry>),
  Components(Vec<Component>),
  Autostart(Vec<AutostartEntry>),
  Diagnostics(Vec<DiagnosticsEntry>),
  Logs(Vec<JournalEntry>),
  Action(String),
}

pub struct SessionApp {
  pub page: SessionPage,
  selected: usize,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  components: Vec<Component>,
  autostart: Vec<AutostartEntry>,
  diagnostics: Vec<DiagnosticsEntry>,
  logs: Vec<JournalEntry>,
  log_filter: Option<String>,
  job: Option<JobHandle<JobData>>,
  action: Option<JobHandle<JobData>>,
  pending: Option<Pending>,
  confirmation: ConfirmationState,
  manager: JobManager,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
}

impl SessionApp {
  pub fn reload(&mut self) {
    self.refresh();
  }

  pub fn new(lang: Lang, theme: Theme) -> Self {
    Self {
      page: SessionPage::Home,
      selected: 0,
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      components: vec![],
      autostart: vec![],
      diagnostics: vec![],
      logs: vec![],
      log_filter: None,
      job: None,
      action: None,
      pending: None,
      confirmation: ConfirmationState::default(),
      manager: JobManager::default(),
      lang,
      theme,
      status: None,
    }
  }

  fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    let page = self.page;
    let filter = self.log_filter.clone();
    let home = self.page == SessionPage::Home;
    self.job = Some(self.manager.spawn(move |_| match page {
      SessionPage::Components => Ok(JobData::Components(backend::list_components())),
      SessionPage::Autostart => Ok(JobData::Autostart(backend::autostart_entries())),
      SessionPage::Diagnostics => {
        let components = backend::list_components();
        Ok(JobData::Diagnostics(backend::diagnostics(&components)))
      }
      SessionPage::Logs | SessionPage::LogDetail(_) => {
        backend::logs(filter.as_deref(), 200).map(JobData::Logs)
      }
      _ if home => {
        let (components, autostart) =
          rayon::join(backend::list_components, backend::autostart_entries);
        Ok(JobData::HomeSummary(components, autostart))
      }
      _ => Ok(JobData::Components(backend::list_components())),
    }));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Carregando sessão...", "Loading session...").into(),
    });
  }

  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(JobData::HomeSummary(components, autostart)) => {
          self.components = merge_component_order(components);
          self.autostart = autostart;
          self.selected = self.selected.min(self.selection_len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Components(components)) => {
          self.components = merge_component_order(components);
          self.selected = self.selected.min(self.selection_len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Autostart(entries)) => {
          self.autostart = entries;
          self.selected = self.selected.min(self.selection_len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Diagnostics(entries)) => {
          self.diagnostics = entries;
          self.selected = self.selected.min(self.selection_len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Logs(entries)) => {
          self.logs = entries;
          self.selected = self.selected.min(self.logs.len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Action(text)) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text,
          });
          self.refresh();
        }
        Err(e) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: e,
          })
        }
      };
      changed = true;
    }
    if let Some(job) = &self.action
      && let JobState::Finished(result) = job.try_state()
    {
      self.action = None;
      match result {
        Ok(JobData::Action(text)) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text,
          });
          self.refresh();
        }
        Ok(_) => {}
        Err(e) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: e,
          });
          self.refresh();
        }
      };
      changed = true;
    }
    changed
  }

  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.pending.is_none() && self.action.is_none() && self.job.is_none() && self.on_buttons {
      match key {
        KeyCode::Tab | KeyCode::BackTab => {
          self.toggle_buttons(key == KeyCode::BackTab);
          return false;
        }
        KeyCode::Left | KeyCode::Char('h') => {
          self.move_button(-1);
          return false;
        }
        KeyCode::Right | KeyCode::Char('l') => {
          self.move_button(1);
          return false;
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
          self.activate_button();
          return false;
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => return false,
        _ => {}
      }
    }
    if matches!(key, KeyCode::Esc | KeyCode::Left) && self.pending.is_none() {
      if self.page == SessionPage::Home {
        return true;
      }
      self.page = if matches!(self.page, SessionPage::LogDetail(_)) {
        SessionPage::Logs
      } else {
        SessionPage::Home
      };
      self.selected = 0;
      self.on_buttons = false;
      self.button_from = None;
      if self.page == SessionPage::Home {
        self.refresh();
      }
      return false;
    }
    if self.job.is_some() || self.action.is_some() {
      return false;
    }
    if self.pending.is_some() {
      match self.confirmation.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.confirmation = ConfirmationState::default();
          let pending = self.pending.take().unwrap();
          self.status = Some(StatusMessage {
            kind: StatusKind::Info,
            text: tr(self.lang, "Aplicando...", "Applying...").into(),
          });
          let lang = self.lang;
          self.action = Some(self.manager.spawn(move |_| match pending {
            Pending::Restart(id) => {
              backend::restart_component(&id)?;
              Ok(JobData::Action(format!(
                "{} '{id}'",
                tr(lang, "Reiniciado", "Restarted")
              )))
            }
            Pending::ToggleAutostart(id, enabled) => {
              backend::set_autostart(&id, enabled)?;
              Ok(JobData::Action(format!(
                "{} '{id}'",
                if enabled {
                  tr(lang, "Ativado", "Enabled")
                } else {
                  tr(lang, "Desativado", "Disabled")
                }
              )))
            }
          }));
        }
        ConfirmationOutcome::Cancelled => {
          self.pending = None;
          self.confirmation = ConfirmationState::default();
        }
        ConfirmationOutcome::Pending => {}
      }
      return false;
    }
    match key {
      KeyCode::Tab | KeyCode::BackTab => self.toggle_buttons(key == KeyCode::BackTab),
      KeyCode::Char('r') => self.refresh(),
      KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
      KeyCode::Down | KeyCode::Char('j') => self.selected = self.selected.saturating_add(1),
      KeyCode::Home => self.selected = 0,
      KeyCode::End => self.selected = self.selection_len().saturating_sub(1),
      KeyCode::PageUp => self.selected = self.selected.saturating_sub(8),
      KeyCode::PageDown => {
        self.selected = self
          .selected
          .saturating_add(8)
          .min(self.selection_len().saturating_sub(1))
      }
      KeyCode::Enter | KeyCode::Right => self.open(),
      _ => {}
    }
    self.selected = self.selected.min(self.selection_len().saturating_sub(1));
    false
  }

  fn selection_len(&self) -> usize {
    match self.page {
      SessionPage::Home => 4,
      SessionPage::Components => self.components.len(),
      SessionPage::Autostart => self.autostart.len(),
      SessionPage::Diagnostics => self.diagnostics.len(),
      SessionPage::Logs => self.logs.len(),
      SessionPage::LogDetail(_) => 1,
    }
  }

  fn open(&mut self) {
    match self.page {
      SessionPage::Home => {
        self.page = [
          SessionPage::Components,
          SessionPage::Autostart,
          SessionPage::Diagnostics,
          SessionPage::Logs,
        ][self.selected.min(3)];
        self.selected = 0;
        self.refresh();
      }
      SessionPage::Components => {
        if let Some(id) = self.components.get(self.selected).map(|c| c.id.clone()) {
          self.pending = Some(Pending::Restart(id));
        }
      }
      SessionPage::Autostart => {
        if let Some(entry) = self.autostart.get(self.selected) {
          self.pending = Some(Pending::ToggleAutostart(entry.id.clone(), !entry.enabled));
        }
      }
      SessionPage::Logs => {
        if self.logs.get(self.selected).is_some() {
          self.page = SessionPage::LogDetail(self.selected);
        }
      }
      SessionPage::Diagnostics | SessionPage::LogDetail(_) => {}
    }
  }

  fn toggle_log_filter(&mut self) {
    let mut units: Vec<String> = self
      .components
      .iter()
      .filter_map(|component| component.unit.clone())
      .collect();
    units.extend(self.logs.iter().filter_map(|entry| entry.unit.clone()));
    units.sort();
    units.dedup();
    if units.is_empty() {
      return;
    }
    self.log_filter = match self.log_filter.as_deref() {
      None => units.first().cloned(),
      Some(current) => units
        .iter()
        .position(|unit| unit == current)
        .and_then(|index| units.get(index + 1).cloned()),
    };
    self.selected = 0;
    self.refresh();
  }

  fn toggle_buttons(&mut self, backwards: bool) {
    let count = self.buttons().len();
    if count == 0 {
      return;
    }
    if self.on_buttons {
      self.on_buttons = false;
      if let Some(index) = self.button_from.take() {
        self.selected = index;
      }
    } else {
      self.button_from = Some(self.selected);
      self.button_selected = if backwards { count - 1 } else { 0 };
      self.on_buttons = true;
    }
  }

  fn move_button(&mut self, delta: isize) {
    let count = self.buttons().len();
    if count == 0 {
      return;
    }
    self.button_selected =
      (self.button_selected as isize + delta).rem_euclid(count as isize) as usize;
  }

  fn activate_button(&mut self) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let actions = self.buttons();
    let Some((button, _)) = actions.get(self.button_selected) else {
      return;
    };
    match button {
      SessionButton::Restart => {
        if let Some(id) = self.components.get(self.selected).map(|c| c.id.clone()) {
          self.pending = Some(Pending::Restart(id));
        }
      }
      SessionButton::Enable => {
        if let Some(id) = self.autostart.get(self.selected).map(|e| e.id.clone()) {
          self.pending = Some(Pending::ToggleAutostart(id, true));
        }
      }
      SessionButton::Disable => {
        if let Some(id) = self.autostart.get(self.selected).map(|e| e.id.clone()) {
          self.pending = Some(Pending::ToggleAutostart(id, false));
        }
      }
      SessionButton::NextLogFilter => self.toggle_log_filter(),
      SessionButton::Refresh => self.refresh(),
    }
  }

  fn buttons(&self) -> Vec<(SessionButton, Button)> {
    let primary = ButtonKind::Primary;
    let secondary = ButtonKind::Secondary;
    match self.page {
      SessionPage::Components => vec![
        (
          SessionButton::Restart,
          Button::new(tr(self.lang, "Reiniciar", "Restart"), primary),
        ),
        (
          SessionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), secondary),
        ),
      ],
      SessionPage::Autostart => vec![
        (
          SessionButton::Enable,
          Button::new(tr(self.lang, "Ativar", "Enable"), primary),
        ),
        (
          SessionButton::Disable,
          Button::new(tr(self.lang, "Desativar", "Disable"), secondary),
        ),
        (
          SessionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), secondary),
        ),
      ],
      SessionPage::Logs => vec![
        (
          SessionButton::NextLogFilter,
          Button::new(
            format!(
              "{}: {}",
              tr(self.lang, "Serviço", "Service"),
              self.log_filter.as_deref().unwrap_or("—")
            ),
            secondary,
          ),
        ),
        (
          SessionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), secondary),
        ),
      ],
      SessionPage::Home | SessionPage::Diagnostics | SessionPage::LogDetail(_) => Vec::new(),
    }
  }

  fn footer_hints(&self) -> &'static str {
    match self.page {
      SessionPage::Home | SessionPage::Diagnostics => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   r Refresh   ←/Esc Back   ? Help",
      ),
      SessionPage::LogDetail(_) => tr(
        self.lang,
        "↑/↓ Rolar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Scroll   ←/Esc Back   ? Help",
      ),
      _ => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   →/Enter Ativar   ←/→ Mover   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   →/Enter Activate   ←/→ Move   r Refresh   ←/Esc Back   ? Help",
      ),
    }
  }

  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Sessão", "Session");
    if self.page == SessionPage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.page_label())
    }
  }

  fn page_label(&self) -> String {
    match self.page {
      SessionPage::Home => tr(self.lang, "Sessão", "Session").into(),
      SessionPage::Components => tr(self.lang, "Componentes", "Components").into(),
      SessionPage::Autostart => tr(self.lang, "Autostart", "Autostart").into(),
      SessionPage::Diagnostics => tr(self.lang, "Diagnóstico", "Diagnostics").into(),
      SessionPage::Logs => tr(self.lang, "Logs", "Logs").into(),
      SessionPage::LogDetail(_) => tr(self.lang, "Logs > Detalhes", "Logs > Details").into(),
    }
  }

  fn home_rows(&self) -> Vec<String> {
    let running = self
      .components
      .iter()
      .filter(|component| component.status == ComponentStatus::Running)
      .count();
    let failed = self
      .components
      .iter()
      .filter(|component| component.status == ComponentStatus::Failed)
      .count();
    let autostart_enabled = self.autostart.iter().filter(|entry| entry.enabled).count();
    vec![
      format!(
        "{} {}  ·  {} {}",
        AppConfig::icon("🧩"),
        tr(self.lang, "Componentes", "Components"),
        running,
        tr(self.lang, "ativos", "active"),
      ),
      format!(
        "{} {}  ·  {} {}",
        AppConfig::icon("🚀"),
        tr(self.lang, "Autostart", "Autostart"),
        autostart_enabled,
        tr(self.lang, "ativados", "enabled"),
      ),
      format!(
        "{} {}  ·  {} {}",
        AppConfig::icon("🩺"),
        tr(self.lang, "Diagnóstico", "Diagnostics"),
        failed,
        tr(self.lang, "com falha", "failed"),
      ),
      format!(
        "{} {}",
        AppConfig::icon("📜"),
        tr(self.lang, "Logs", "Logs")
      ),
    ]
  }

  fn diagnostics_rows(&self) -> Vec<String> {
    if self.job.is_some() && self.diagnostics.is_empty() {
      return vec![
        tr(
          self.lang,
          "Carregando diagnóstico...",
          "Loading diagnostics...",
        )
        .into(),
      ];
    }
    self
      .diagnostics
      .iter()
      .map(|entry| {
        let symbol = if entry.warn {
          "⚠"
        } else if entry.ok {
          "✓"
        } else {
          "✕"
        };
        format!(" {symbol} {}  ·  {}", entry.label, entry.detail)
      })
      .collect()
  }

  fn component_rows(&self) -> Vec<String> {
    self
      .components
      .iter()
      .map(|component| component_row(self.lang, component))
      .collect()
  }

  fn autostart_rows(&self) -> Vec<String> {
    self
      .autostart
      .iter()
      .map(|entry| autostart_row(self.lang, entry))
      .collect()
  }

  fn log_rows(&self) -> Vec<String> {
    self.logs.iter().map(log_line).collect()
  }

  fn log_detail_lines(&self, index: usize) -> Vec<Line<'static>> {
    let Some(entry) = self.logs.get(index) else {
      return vec![Line::from(tr(
        self.lang,
        "Log não encontrado",
        "Log not found",
      ))];
    };
    vec![
      Line::from(format!(
        "{}: {}",
        tr(self.lang, "Horário", "Timestamp"),
        entry.rendered_timestamp()
      )),
      Line::from(format!(
        "{}: {}",
        tr(self.lang, "Unidade", "Unit"),
        entry.unit.as_deref().unwrap_or("—")
      )),
      Line::from(format!("PID: {}", entry.pid.as_deref().unwrap_or("—"))),
      Line::from(""),
      Line::from(entry.message.clone().unwrap_or_default()),
    ]
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let body = shell(
      frame,
      area,
      &self.theme,
      &self.breadcrumb(),
      self.footer_hints(),
    );
    let lines = match self.page {
      SessionPage::Home => self.home_rows(),
      SessionPage::Components => self.component_rows(),
      SessionPage::Autostart => self.autostart_rows(),
      SessionPage::Diagnostics => self.diagnostics_rows(),
      SessionPage::Logs => self.log_rows(),
      SessionPage::LogDetail(_) => Vec::new(),
    };
    if let SessionPage::LogDetail(index) = self.page {
      let entries = self.log_detail_lines(index);
      readonly(frame, body, &self.theme, &entries);
      return;
    }
    let buttons = self.buttons();
    let raw_buttons: Vec<Button> = buttons.into_iter().map(|(_, button)| button).collect();
    let (body, button_area) = if raw_buttons.is_empty() {
      (body, None)
    } else {
      let button_height = argvus_tui::buttons::height(&raw_buttons, body.width).min(body.height);
      let split =
        Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(body);
      (split[0], Some(split[1]))
    };
    let list_selection = if self.job.is_some() {
      usize::MAX
    } else {
      self.selected.min(lines.len().saturating_sub(1))
    };
    list(frame, body, &self.theme, &lines, list_selection);
    if let Some(button_area) = button_area {
      let focus = if self.on_buttons {
        self.button_selected
      } else {
        usize::MAX
      };
      argvus_tui::buttons::draw(frame, button_area, &raw_buttons, focus, &self.theme);
    }
    if let Some(s) = &self.status {
      status(frame, area, &self.theme, s);
    }
    if let Some(pending) = &self.pending {
      self.draw_pending(frame, area, pending);
    }
  }

  fn draw_pending(&self, frame: &mut Frame, area: Rect, pending: &Pending) {
    let (title, message) = match pending {
      Pending::Restart(id) => (
        tr(self.lang, "Confirmar reinício", "Confirm restart"),
        format!(
          "{} {id}?",
          tr(self.lang, "Reiniciar componente", "Restart component")
        ),
      ),
      Pending::ToggleAutostart(id, enabled) => (
        tr(self.lang, "Confirmar autostart", "Confirm autostart"),
        format!(
          "{} {id}?",
          if *enabled {
            tr(self.lang, "Ativar", "Enable")
          } else {
            tr(self.lang, "Desativar", "Disable")
          }
        ),
      ),
    };
    draw_confirmation(
      frame,
      area,
      &self.theme,
      ConfirmationDialog {
        title,
        message: &message,
        confirm_label: tr(self.lang, "Continuar", "Continue"),
        cancel_label: tr(self.lang, "Cancelar", "Cancel"),
        confirm_selected: self.confirmation.confirm_selected,
      },
    )
  }
}

fn merge_component_order(components: Vec<Component>) -> Vec<Component> {
  let order: Vec<String> = manifest()
    .into_iter()
    .map(|component| component.id)
    .collect();
  let mut sorted = components;
  sorted.sort_by_key(|component| {
    order
      .iter()
      .position(|id| id == &component.id)
      .unwrap_or(usize::MAX)
  });
  sorted
}

fn component_row(lang: Lang, component: &Component) -> String {
  let (symbol, label) = match component.status {
    ComponentStatus::Running => ("●", tr(lang, "Ativo", "Active")),
    ComponentStatus::Stopped => ("○", tr(lang, "Parado", "Stopped")),
    ComponentStatus::Failed => ("✕", tr(lang, "Falho", "Failed")),
  };
  let role = if component.essential {
    tr(lang, "essencial", "essential")
  } else {
    tr(lang, "opcional", "optional")
  };
  let pid = component
    .pid
    .map(|pid| pid.to_string())
    .unwrap_or_else(|| "—".into());
  format!(
    "{}  {}  {symbol} {}  ·  PID {}  ·  {}",
    component.display_name, role, label, pid, component.description
  )
}

fn autostart_row(lang: Lang, entry: &AutostartEntry) -> String {
  let symbol = if entry.enabled { "●" } else { "○" };
  let state = if entry.enabled {
    tr(lang, "Ativado", "Enabled")
  } else {
    tr(lang, "Desativado", "Disabled")
  };
  let origin = if entry.from_system {
    tr(lang, "sistema", "system")
  } else {
    tr(lang, "usuário", "user")
  };
  format!(
    "{}  {symbol} {} · {}: {} · {}",
    entry.name,
    state,
    tr(lang, "origem", "source"),
    origin,
    entry.command
  )
}

fn log_line(entry: &JournalEntry) -> String {
  format!(
    "{:8} {:28} {:7} {}",
    entry.rendered_timestamp(),
    entry.unit.as_deref().unwrap_or("user"),
    entry.pid.as_deref().unwrap_or("—"),
    entry.message.as_deref().unwrap_or("")
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn component(id: &str, status: ComponentStatus) -> Component {
    Component {
      id: id.into(),
      display_name: id.into(),
      description: String::new(),
      process: vec![],
      unit: None,
      essential: false,
      status,
      pid: Some(42),
    }
  }

  #[test]
  fn home_rows_act_as_dashboard() {
    let mut app = SessionApp::new(Lang::En, Theme::load());
    app.components = vec![
      component("a", ComponentStatus::Running),
      component("b", ComponentStatus::Running),
      component("c", ComponentStatus::Failed),
    ];
    app.autostart = vec![AutostartEntry {
      id: "x".into(),
      name: "X".into(),
      command: "x".into(),
      from_system: true,
      enabled: true,
      path: "/tmp/x.desktop".into(),
    }];
    let rows = app.home_rows();
    assert_eq!(rows.len(), 4);
    assert!(rows[0].contains("2 active"));
    assert!(rows[1].contains("1 enabled"));
    assert!(rows[2].contains("1 failed"));
  }

  #[test]
  fn component_rows_show_state_badges() {
    let rows = [
      component("a", ComponentStatus::Running),
      component("b", ComponentStatus::Failed),
    ]
    .iter()
    .map(|component| component_row(Lang::En, component))
    .collect::<Vec<_>>();
    assert!(rows[0].contains("●") && rows[0].contains("Active"));
    assert!(rows[1].contains("✕") && rows[1].contains("Failed"));
  }

  #[test]
  fn autostart_rows_show_origin() {
    let entry = AutostartEntry {
      id: "sys".into(),
      name: "Sys".into(),
      command: "run --x".into(),
      from_system: true,
      enabled: false,
      path: "/etc/xdg/autostart/sys.desktop".into(),
    };
    let row = autostart_row(Lang::En, &entry);
    assert!(row.contains("Disabled"));
    assert!(row.contains("system"));
  }

  #[test]
  fn order_follows_manifest() {
    let components = merge_component_order(vec![
      component("hyprpaper", ComponentStatus::Running),
      component("argvus-session", ComponentStatus::Running),
    ]);
    assert_eq!(components[0].id, "argvus-session");
  }

  #[test]
  fn session_app_navigates_and_opens_pages() {
    let mut app = SessionApp::new(Lang::En, Theme::load());
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, SessionPage::Autostart);
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, SessionPage::Home);
  }

  #[test]
  fn session_app_renders_components_page() {
    let mut app = SessionApp::new(Lang::En, Theme::load());
    app.page = SessionPage::Components;
    app.components = vec![
      component("waybar", ComponentStatus::Running),
      component("hypridle", ComponentStatus::Stopped),
    ];
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("waybar") && (text.contains("Restart") || text.contains("Reiniciar")));
  }

  #[test]
  fn session_app_renders_home() {
    let mut app = SessionApp::new(Lang::En, Theme::load());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
  }

  #[test]
  fn diagnostics_rows_render_symbols() {
    let mut app = SessionApp::new(Lang::En, Theme::load());
    app.diagnostics = vec![
      DiagnosticsEntry::good("A", "ok"),
      DiagnosticsEntry::warning("B", "warn"),
      DiagnosticsEntry::bad("C", "bad"),
    ];
    let rows = app.diagnostics_rows();
    assert!(rows[0].starts_with(" ✓ "));
    assert!(rows[1].starts_with(" ⚠ "));
    assert!(rows[2].starts_with(" ✕ "));
  }
}
