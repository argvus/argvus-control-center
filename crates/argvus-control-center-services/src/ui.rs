use crate::{
  backend,
  journal::JournalEntry,
  model::{ServicePage, Unit},
};
use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedOperation, PrivilegedRequest, SystemSettingsOperation},
  process::{ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
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
use ratatui::{
  Frame,
  layout::{Constraint, Layout},
  text::Line,
};

#[derive(Debug, Clone)]
enum Pending {
  Action(String, String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  Start,
  Stop,
  Restart,
  Filter,
  PrevBoot,
  NextUnit,
  Priority,
}
pub struct ServicesApp {
  pub page: ServicePage,
  detail_parent: ServicePage,
  detail_unit: Option<String>,
  selected: usize,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  units: Vec<Unit>,
  logs: Vec<JournalEntry>,
  search: String,
  searching: bool,
  job: Option<JobHandle<JobData>>,
  action: Option<JobHandle<String>>,
  pending: Option<Pending>,
  confirmation: ConfirmationState,
  manager: JobManager,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
  previous_boot: bool,
  priority: Option<String>,
  logs_unit: Option<String>,
  filter: UnitFilter,
  log_search: String,
}
enum JobData {
  Units(Vec<Unit>),
  Logs(Vec<JournalEntry>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitFilter {
  All,
  Running,
  Stopped,
  Failed,
  Enabled,
  Disabled,
}
impl UnitFilter {
  fn next(self) -> Self {
    match self {
      Self::All => Self::Running,
      Self::Running => Self::Stopped,
      Self::Stopped => Self::Failed,
      Self::Failed => Self::Enabled,
      Self::Enabled => Self::Disabled,
      Self::Disabled => Self::All,
    }
  }
  fn label(self, lang: Lang) -> &'static str {
    match self {
      Self::All => tr(lang, "Todos", "All"),
      Self::Running => tr(lang, "Ativos", "Running"),
      Self::Stopped => tr(lang, "Parados", "Stopped"),
      Self::Failed => tr(lang, "Falhos", "Failed"),
      Self::Enabled => tr(lang, "Habilitados", "Enabled"),
      Self::Disabled => tr(lang, "Desabilitados", "Disabled"),
    }
  }
}
impl ServicesApp {
  pub fn reload(&mut self) {
    self.refresh();
  }
  pub fn new(lang: Lang, theme: Theme) -> Self {
    Self {
      page: ServicePage::Home,
      detail_parent: ServicePage::System,
      detail_unit: None,
      selected: 0,
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      units: vec![],
      logs: vec![],
      search: String::new(),
      searching: false,
      job: None,
      action: None,
      pending: None,
      confirmation: ConfirmationState::default(),
      manager: JobManager::default(),
      lang,
      theme,
      status: None,
      previous_boot: false,
      priority: None,
      logs_unit: None,
      filter: UnitFilter::All,
      log_search: String::new(),
    }
  }
  fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    let page = self.page;
    let previous = self.previous_boot;
    let priority = self.priority.clone();
    let logs_unit = self.logs_unit.clone();
    let detail_user = self.detail_parent == ServicePage::User;
    self.job = Some(self.manager.spawn(move |_| match page {
      ServicePage::Home => {
        let (system, user) = rayon::join(|| backend::list(false), || backend::list(true));
        let mut system = system?;
        system.append(&mut user?);
        Ok(JobData::Units(system))
      }
      ServicePage::User => backend::list(true).map(JobData::Units),
      ServicePage::Failed => {
        let (system, user) = rayon::join(|| backend::list(false), || backend::list(true));
        let mut system = system?;
        let mut user = user?;
        system.append(&mut user);
        Ok(JobData::Units(system))
      }
      ServicePage::Logs | ServicePage::LogDetail(_) => {
        backend::logs(logs_unit.as_deref(), previous, priority.as_deref(), 200).map(JobData::Logs)
      }
      ServicePage::Detail => backend::list(detail_user).map(JobData::Units),
      _ => backend::list(false).map(JobData::Units),
    }));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Carregando serviços...", "Loading services...").into(),
    });
  }
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(JobData::Units(v)) => {
          self.units = v;
          self.selected = self.selected.min(self.filtered().len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Logs(v)) => {
          self.logs = v;
          self.selected = self.selected.min(self.logs.len().saturating_sub(1));
          self.status = None;
        }
        Err(e) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: e,
          })
        }
      };
      changed = true
    }
    if let Some(job) = &self.action
      && let JobState::Finished(result) = job.try_state()
    {
      self.action = None;
      match result {
        Ok(v) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: v,
          })
        }
        Err(e) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: e,
          })
        }
      };
      self.refresh();
      changed = true
    }
    changed
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.pending.is_none()
      && !self.searching
      && self.action.is_none()
      && self.job.is_none()
      && self.on_buttons
      && !self.buttons().is_empty()
    {
      match key {
        KeyCode::Tab => {
          self.toggle_buttons(false);
          return false;
        }
        KeyCode::BackTab => {
          self.toggle_buttons(true);
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
    if matches!(key, KeyCode::Esc | KeyCode::Left) && self.pending.is_none() && !self.searching {
      if self.page == ServicePage::Home {
        return true;
      }
      self.page = if self.page == ServicePage::Detail {
        self.detail_parent
      } else if matches!(self.page, ServicePage::LogDetail(_)) {
        ServicePage::Logs
      } else {
        ServicePage::Home
      };
      self.selected = 0;
      self.on_buttons = false;
      self.button_from = None;
      if self.page == ServicePage::Home {
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
          let Pending::Action(a, u) = self.pending.take().unwrap();
          let user = self.page == ServicePage::User
            || self.detail_parent == ServicePage::User
            || self
              .units
              .iter()
              .any(|unit| unit.name == u && unit.scope == "user");
          self.status = Some(StatusMessage {
            kind: StatusKind::Info,
            text: tr(self.lang, "Aplicando ação...", "Applying action...").into(),
          });
          self.action = Some(self.manager.spawn(move |_| run_action(user, &a, &u)));
        }
        ConfirmationOutcome::Cancelled => {
          self.pending = None;
          self.confirmation = ConfirmationState::default();
        }
        ConfirmationOutcome::Pending => {}
      }
      return false;
    }
    if self.searching {
      let logs = self.page == ServicePage::Logs;
      match key {
        KeyCode::Esc => self.searching = false,
        KeyCode::Backspace => {
          if logs {
            self.log_search.pop();
          } else {
            self.search.pop();
          }
          self.selected = 0
        }
        KeyCode::Enter => self.searching = false,
        KeyCode::Char(c) => {
          if logs {
            self.log_search.push(c);
          } else {
            self.search.push(c);
          }
        }
        KeyCode::Down => self.selected += 1,
        KeyCode::Up => self.selected = self.selected.saturating_sub(1),
        _ => {}
      }
      let count = if logs {
        self.filtered_logs().len()
      } else {
        self.filtered().len()
      };
      self.selected = self.selected.min(count.saturating_sub(1));
      return false;
    }
    match key {
      KeyCode::Tab | KeyCode::BackTab => self.toggle_buttons(key == KeyCode::BackTab),
      KeyCode::Char('r') => self.refresh(),
      KeyCode::Char('/')
        if matches!(
          self.page,
          ServicePage::System | ServicePage::User | ServicePage::Failed | ServicePage::Logs
        ) =>
      {
        self.searching = true
      }
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
    let count = match self.page {
      ServicePage::Home => 4,
      ServicePage::System | ServicePage::User | ServicePage::Failed => self.filtered().len(),
      ServicePage::Logs => self.filtered_logs().len(),
      ServicePage::Detail => self.detail_actions().len(),
      _ => 1,
    };
    self.selected = self.selected.min(count.saturating_sub(1));
    false
  }
  fn selection_len(&self) -> usize {
    match self.page {
      ServicePage::Home => 4,
      ServicePage::System | ServicePage::User | ServicePage::Failed => self.filtered().len(),
      ServicePage::Logs => self.logs.len(),
      ServicePage::Detail => self.detail_actions().len(),
      ServicePage::LogDetail(_) => 1,
    }
  }
  fn open(&mut self) {
    match self.page {
      ServicePage::Home => {
        self.page = [
          ServicePage::System,
          ServicePage::User,
          ServicePage::Failed,
          ServicePage::Logs,
        ][self.selected.min(3)];
        self.selected = 0;
        self.refresh()
      }
      ServicePage::System | ServicePage::User | ServicePage::Failed => {
        if self.filtered().get(self.selected).is_some() {
          self.detail_parent = self.page;
          self.detail_unit = self
            .filtered()
            .get(self.selected)
            .map(|unit| unit.name.clone());
          self.page = ServicePage::Detail
        }
      }
      ServicePage::Detail => {
        if let Some((action, _)) = self.detail_actions().get(self.selected) {
          if *action == "logs" {
            self.open_logs();
          } else {
            let action = *action;
            self.request(action);
          }
        }
      }
      ServicePage::Logs => {
        if self.filtered_logs().get(self.selected).is_some() {
          self.page = ServicePage::LogDetail(self.selected);
        }
      }
      ServicePage::LogDetail(_) => {}
    }
  }
  fn request(&mut self, a: &str) {
    if matches!(
      self.page,
      ServicePage::System | ServicePage::User | ServicePage::Failed | ServicePage::Detail
    ) && let Some(u) = self.detail_or_selected_unit()
      && can_action(a, u)
    {
      self.pending = Some(Pending::Action(a.into(), u.name.clone()))
    }
  }
  fn open_logs(&mut self) {
    let unit = self
      .detail_unit
      .clone()
      .or_else(|| self.filtered().get(self.selected).map(|u| u.name.clone()));
    if let Some(unit) = unit {
      self.page = ServicePage::Logs;
      self.logs_unit = Some(unit.clone());
      self.job = Some(
        self
          .manager
          .spawn(move |_| backend::logs(Some(&unit), false, None, 200).map(JobData::Logs)),
      );
      self.selected = 0
    }
  }
  fn filtered(&self) -> Vec<&Unit> {
    self
      .units
      .iter()
      .filter(|u| self.page != ServicePage::Failed || u.failed())
      .filter(|u| match self.filter {
        UnitFilter::All => true,
        UnitFilter::Running => u.active == "active",
        UnitFilter::Stopped => u.active == "inactive",
        UnitFilter::Failed => u.failed(),
        UnitFilter::Enabled => matches!(u.file_state.as_str(), "enabled" | "static" | "indirect"),
        UnitFilter::Disabled => matches!(u.file_state.as_str(), "disabled" | "masked"),
      })
      .filter(|u| {
        self.search.is_empty()
          || u
            .name
            .to_ascii_lowercase()
            .contains(&self.search.to_ascii_lowercase())
          || u
            .description
            .to_ascii_lowercase()
            .contains(&self.search.to_ascii_lowercase())
      })
      .collect()
  }
  fn filtered_logs(&self) -> Vec<&JournalEntry> {
    let query = self.log_search.to_ascii_lowercase();
    self
      .logs
      .iter()
      .filter(|entry| {
        query.is_empty()
          || entry
            .message
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(&query)
          || entry
            .unit
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(&query)
      })
      .collect()
  }
  fn cycle_log_unit(&mut self) {
    let mut units: Vec<String> = self
      .logs
      .iter()
      .filter_map(|entry| entry.unit.clone())
      .filter(|unit| !unit.is_empty())
      .collect();
    units.sort();
    units.dedup();
    if units.is_empty() {
      return;
    }
    self.logs_unit = match self.logs_unit.as_deref() {
      None => units.first().cloned(),
      Some(current) => units
        .iter()
        .position(|unit| unit == current)
        .and_then(|index| units.get(index + 1).cloned()),
    };
    self.selected = 0;
    self.refresh();
  }
  pub fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Serviços", "Services");
    if self.page == ServicePage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.label(self.page))
    }
  }
  fn label(&self, p: ServicePage) -> &'static str {
    match p {
      ServicePage::System => tr(self.lang, "Sistema", "System"),
      ServicePage::User => tr(self.lang, "Usuário", "User"),
      ServicePage::Failed => tr(self.lang, "Falhos", "Failed"),
      ServicePage::Logs => tr(self.lang, "Logs", "Logs"),
      ServicePage::Detail => tr(self.lang, "Detalhes", "Details"),
      ServicePage::LogDetail(_) => tr(self.lang, "Logs > Detalhes", "Logs > Details"),
      ServicePage::Home => tr(self.lang, "Serviços", "Services"),
    }
  }
  fn cycle_filter(&mut self) {
    self.filter = self.filter.next();
    self.selected = 0;
  }
  fn cycle_priority(&mut self) {
    self.priority = match self.priority.as_deref() {
      None | Some("7") => None,
      Some(value) => Some((value.parse::<u8>().unwrap_or(0) + 1).to_string()),
    };
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
    let Some((action, _)) = actions.get(self.button_selected) else {
      return;
    };
    match *action {
      ActionButton::Start => self.request("start"),
      ActionButton::Stop => self.request("stop"),
      ActionButton::Restart => self.request("restart"),
      ActionButton::Filter => self.cycle_filter(),
      ActionButton::PrevBoot => {
        self.previous_boot = !self.previous_boot;
        self.refresh();
      }
      ActionButton::NextUnit => self.cycle_log_unit(),
      ActionButton::Priority => self.cycle_priority(),
    }
  }
  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    let primary = ButtonKind::Primary;
    let secondary = ButtonKind::Secondary;
    match self.page {
      ServicePage::System | ServicePage::User | ServicePage::Failed => vec![
        (
          ActionButton::Start,
          Button::new(tr(self.lang, "Iniciar", "Start"), primary),
        ),
        (
          ActionButton::Stop,
          Button::new(tr(self.lang, "Parar", "Stop"), secondary),
        ),
        (
          ActionButton::Restart,
          Button::new(tr(self.lang, "Reiniciar", "Restart"), secondary),
        ),
        (
          ActionButton::Filter,
          Button::new(
            format!(
              "{}: {}",
              tr(self.lang, "Filtro", "Filter"),
              self.filter.label(self.lang)
            ),
            secondary,
          ),
        ),
      ],
      ServicePage::Logs => vec![
        (
          ActionButton::PrevBoot,
          Button::new(
            format!(
              "{}: {}",
              tr(self.lang, "Boot", "Boot"),
              if self.previous_boot {
                tr(self.lang, "Anterior", "Previous")
              } else {
                tr(self.lang, "Atual", "Current")
              }
            ),
            secondary,
          ),
        ),
        (
          ActionButton::NextUnit,
          Button::new(
            format!(
              "{}: {}",
              tr(self.lang, "Serviço", "Service"),
              self.logs_unit.as_deref().unwrap_or("—")
            ),
            secondary,
          ),
        ),
        (
          ActionButton::Priority,
          Button::new(
            format!(
              "{}: {}",
              tr(self.lang, "Prioridade", "Priority"),
              self.priority.as_deref().unwrap_or("0-7")
            ),
            secondary,
          ),
        ),
      ],
      ServicePage::Home | ServicePage::Detail | ServicePage::LogDetail(_) => Vec::new(),
    }
  }
  fn footer_hints(&self) -> &'static str {
    match self.page {
      ServicePage::Detail => tr(
        self.lang,
        "↑/↓ Navegar   Enter Executar   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Run   r Refresh   ←/Esc Back   ? Help",
      ),
      ServicePage::LogDetail(_) => tr(
        self.lang,
        "↑/↓ Rolar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Scroll   ←/Esc Back   ? Help",
      ),
      ServicePage::Home => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   r Refresh   ←/Esc Back   ? Help",
      ),
      _ => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   →/Enter Ativar   ←/→ Mover   / Buscar   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   →/Enter Activate   ←/→ Move   / Search   r Refresh   ←/Esc Back   ? Help",
      ),
    }
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
    if self.page == ServicePage::Detail {
      let sections = Layout::vertical([Constraint::Length(7), Constraint::Min(1)]).split(body);
      readonly(frame, sections[0], &self.theme, &self.detail_lines());
      let actions = self
        .detail_actions()
        .into_iter()
        .map(|(_, label)| label)
        .collect::<Vec<_>>();
      list(
        frame,
        sections[1],
        &self.theme,
        &actions,
        self.selected.min(actions.len().saturating_sub(1)),
      );
      if let Some(s) = &self.status {
        status(frame, area, &self.theme, s)
      }
      if let Some(Pending::Action(a, u)) = &self.pending {
        self.draw_pending(frame, area, a, u);
      }
      return;
    }
    let mut lines: Vec<String> = if self.page == ServicePage::Home {
      self.home_rows()
    } else if self.page == ServicePage::Logs {
      self
        .filtered_logs()
        .iter()
        .map(|entry| log_line(entry))
        .map(|l| l.to_string())
        .collect()
    } else if let ServicePage::LogDetail(index) = self.page {
      self
        .log_detail_lines(index)
        .into_iter()
        .map(|l| l.to_string())
        .collect()
    } else if self.page == ServicePage::Failed && self.filtered().is_empty() {
      vec![format!(
        "[OK] {}",
        tr(self.lang, "Nenhuma unidade com falha.", "No failed units.")
      )]
    } else {
      self.unit_rows()
    };
    if self.page == ServicePage::Logs {
      lines.insert(0, self.logs_header());
    }
    if self.searching {
      lines.insert(
        0,
        format!("{}: {}_", tr(self.lang, "Buscar", "Search"), self.search),
      )
    }
    let visual_offset = usize::from(self.page == ServicePage::Logs) + usize::from(self.searching);
    let buttons = self.buttons();
    let raw_buttons: Vec<Button> = buttons.iter().map(|(_, button)| button.clone()).collect();
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
      self
        .selected
        .saturating_add(visual_offset)
        .min(lines.len().saturating_sub(1))
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
      status(frame, area, &self.theme, s)
    }
    if let Some(Pending::Action(a, u)) = &self.pending {
      self.draw_pending(frame, area, a, u)
    }
  }
  fn draw_pending(&self, frame: &mut Frame, area: ratatui::layout::Rect, action: &str, unit: &str) {
    draw_confirmation(
      frame,
      area,
      &self.theme,
      ConfirmationDialog {
        title: tr(self.lang, "Confirmar ação", "Confirm action"),
        message: &format!("{} {}?", action_label(self.lang, action), unit),
        confirm_label: tr(self.lang, "Continuar", "Continue"),
        cancel_label: tr(self.lang, "Cancelar", "Cancel"),
        confirm_selected: self.confirmation.confirm_selected,
      },
    )
  }
  fn home_rows(&self) -> Vec<String> {
    let (system_active, system_enabled) = self.counts("system");
    let (user_active, user_enabled) = self.counts("user");
    let failed = self.units.iter().filter(|unit| unit.failed()).count();
    let boot = if self.previous_boot {
      tr(self.lang, "Anterior", "Previous")
    } else {
      tr(self.lang, "Atual", "Current")
    };
    vec![
      format!(
        "{} {}  ·  {} {} · {} {}",
        AppConfig::icon("⚙️"),
        tr(self.lang, "Sistema", "System"),
        system_active,
        tr(self.lang, "ativos", "active"),
        system_enabled,
        tr(self.lang, "habilitados", "enabled"),
      ),
      format!(
        "{} {}  ·  {} {} · {} {}",
        AppConfig::icon("👤"),
        tr(self.lang, "Usuário", "User"),
        user_active,
        tr(self.lang, "ativos", "active"),
        user_enabled,
        tr(self.lang, "habilitados", "enabled"),
      ),
      format!(
        "{} {}  ·  {} {}",
        AppConfig::icon("⚠️"),
        tr(self.lang, "Falhos", "Failed"),
        failed,
        tr(self.lang, "com falha", "failed"),
      ),
      format!(
        "{} {}  ·  {} {} · {} {}",
        AppConfig::icon("📜"),
        tr(self.lang, "Logs", "Logs"),
        tr(self.lang, "Boot", "Boot"),
        boot,
        tr(self.lang, "Prioridade", "Priority"),
        self.priority.as_deref().unwrap_or("0-7"),
      ),
    ]
  }
  fn counts(&self, scope: &str) -> (usize, usize) {
    let active = self
      .units
      .iter()
      .filter(|unit| unit.scope == scope && unit.active == "active")
      .count();
    let enabled = self
      .units
      .iter()
      .filter(|unit| {
        unit.scope == scope && matches!(unit.file_state.as_str(), "enabled" | "static" | "indirect")
      })
      .count();
    (active, enabled)
  }
  fn logs_header(&self) -> String {
    let boot = if self.previous_boot {
      tr(self.lang, "Anterior", "Previous")
    } else {
      tr(self.lang, "Atual", "Current")
    };
    format!(
      "{} {}  ·  {} {} · {} {} · {} {}",
      AppConfig::icon("📜"),
      tr(self.lang, "Logs", "Logs"),
      tr(self.lang, "Boot", "Boot"),
      boot,
      tr(self.lang, "Prioridade", "Priority"),
      self.priority.as_deref().unwrap_or("0-7"),
      tr(self.lang, "Serviço", "Service"),
      self.logs_unit.as_deref().unwrap_or("—"),
    )
  }
  fn unit_rows(&self) -> Vec<String> {
    self
      .filtered()
      .iter()
      .map(|unit| self.unit_row(unit))
      .collect()
  }
  fn unit_row(&self, u: &Unit) -> String {
    let mut row = u.name.clone();
    if let Some(badge) = self.active_badge(u) {
      row.push_str(&badge);
    }
    row.push_str(&self.file_badge(u));
    if self.page == ServicePage::Failed && u.scope == "user" {
      row.push_str(&format!("   [{}]", tr(self.lang, "usuário", "user")));
    }
    row
  }
  fn active_badge(&self, u: &Unit) -> Option<String> {
    let (symbol, label) = match u.active.as_str() {
      "active" => ("●", tr(self.lang, "Ativo", "Active")),
      "inactive" => ("○", tr(self.lang, "Inativo", "Inactive")),
      "failed" => ("✕", tr(self.lang, "Falho", "Failed")),
      "activating" => ("◌", tr(self.lang, "Ativando", "Activating")),
      "deactivating" => ("◌", tr(self.lang, "Desativando", "Deactivating")),
      "reloading" => ("◌", tr(self.lang, "Recarregando", "Reloading")),
      _ => return None,
    };
    Some(format!("   {symbol} {label}"))
  }
  fn file_badge(&self, u: &Unit) -> String {
    match u.file_state.as_str() {
      "enabled" | "static" | "indirect" => {
        format!("   ● {}", tr(self.lang, "Habilitado", "Enabled"))
      }
      "disabled" | "masked" => {
        format!("   ○ {}", tr(self.lang, "Desabilitado", "Disabled"))
      }
      _ => String::new(),
    }
  }
  fn detail_lines(&self) -> Vec<Line<'static>> {
    let filtered = self.filtered();
    let Some(u) = self
      .detail_unit
      .as_deref()
      .and_then(|name| self.units.iter().find(|unit| unit.name == name))
      .or_else(|| filtered.get(self.selected).copied())
    else {
      return vec![Line::from(tr(
        self.lang,
        "Serviço não encontrado",
        "Service not found",
      ))];
    };
    let rows = vec![
      Line::from(format!(
        " {} {}",
        AppConfig::icon("⚙️"),
        tr(self.lang, "SERVIÇO", "SERVICE")
      )),
      Line::from(format!(
        "   {:<12} {}",
        tr(self.lang, "Nome:", "Name:"),
        u.name
      )),
      Line::from(format!(
        "   {:<12} {}",
        tr(self.lang, "Descrição:", "Description:"),
        u.description
      )),
      Line::from(format!(
        "   {:<12} {}",
        tr(self.lang, "Estado:", "State:"),
        state_value(u)
      )),
      Line::from(format!(
        "   {:<12} {}",
        tr(self.lang, "Boot:", "Boot:"),
        file_state_label(self.lang, &u.file_state)
      )),
      Line::from(format!(
        "   {:<12} {}",
        tr(self.lang, "PID:", "Main PID:"),
        u.main_pid
          .map(|v| v.to_string())
          .unwrap_or_else(|| "—".into())
      )),
      Line::from(format!(
        "   {:<12} {}",
        tr(self.lang, "Arquivo:", "Unit file:"),
        u.fragment.as_deref().unwrap_or("—")
      )),
    ];
    rows
  }
  fn detail_actions(&self) -> Vec<(&'static str, String)> {
    let Some(unit) = self.detail_or_selected_unit() else {
      return Vec::new();
    };
    let mut actions = [
      "start",
      "stop",
      "restart",
      "enable",
      "disable",
      "enable-now",
      "disable-now",
    ]
    .into_iter()
    .filter(|action| can_action(action, unit))
    .map(|action| (action, action_label(self.lang, action)))
    .collect::<Vec<_>>();
    actions.push(("logs", tr(self.lang, "Logs", "Logs").into()));
    actions
  }
  fn detail_or_selected_unit(&self) -> Option<&Unit> {
    self
      .detail_unit
      .as_deref()
      .and_then(|name| self.units.iter().find(|unit| unit.name == name))
      .or_else(|| self.filtered().get(self.selected).copied())
  }
  fn log_detail_lines(&self, index: usize) -> Vec<Line<'static>> {
    let filtered = self.filtered_logs();
    let Some(entry) = filtered.get(index) else {
      return vec![Line::from(tr(
        self.lang,
        "Log não encontrado",
        "Log not found",
      ))];
    };
    let timestamp = entry.timestamp.clone().unwrap_or_else(|| "—".into());
    let unit = entry.unit.clone().unwrap_or_else(|| "kernel".into());
    let priority = entry.priority_name().to_string();
    let pid = entry.pid.clone().unwrap_or_else(|| "—".into());
    let boot = entry.boot_id.clone().unwrap_or_else(|| "—".into());
    let executable = entry.executable.clone().unwrap_or_else(|| "—".into());
    let message = entry.message.clone().unwrap_or_default();
    vec![
      Line::from(format!(
        "{}: {}",
        tr(self.lang, "Horário", "Timestamp"),
        timestamp
      )),
      Line::from(format!("{}: {}", tr(self.lang, "Unidade", "Unit"), unit)),
      Line::from(format!(
        "{}: {}",
        tr(self.lang, "Prioridade", "Priority"),
        priority
      )),
      Line::from(format!("PID: {}", pid)),
      Line::from(format!(
        "{}: {}",
        tr(self.lang, "Executável", "Executable"),
        executable
      )),
      Line::from(format!("{}: {}", tr(self.lang, "Boot", "Boot"), boot)),
      Line::from(""),
      Line::from(message),
    ]
  }
}
fn can_action(action: &str, unit: &Unit) -> bool {
  match action {
    "start" => !matches!(unit.active.as_str(), "active" | "activating"),
    "stop" => matches!(unit.active.as_str(), "active" | "activating"),
    "restart" => matches!(unit.active.as_str(), "active" | "activating"),
    "enable" | "enable-now" => {
      !matches!(unit.file_state.as_str(), "enabled" | "static" | "indirect")
    }
    "disable" | "disable-now" => matches!(unit.file_state.as_str(), "enabled" | "indirect"),
    _ => false,
  }
}
fn action_label(lang: Lang, action: &str) -> String {
  match action {
    "start" => tr(lang, "Iniciar", "Start"),
    "stop" => tr(lang, "Parar", "Stop"),
    "restart" => tr(lang, "Reiniciar", "Restart"),
    "enable" => tr(lang, "Habilitar", "Enable"),
    "disable" => tr(lang, "Desabilitar", "Disable"),
    "enable-now" => tr(lang, "Habilitar e iniciar", "Enable and start"),
    "disable-now" => tr(lang, "Desabilitar e parar", "Disable and stop"),
    other => other,
  }
  .into()
}
fn state_value(u: &Unit) -> String {
  format!("{} / {}", u.active, u.sub)
}
fn file_state_label(lang: Lang, state: &str) -> String {
  match state {
    "enabled" | "static" | "indirect" => tr(lang, "Habilitado", "Enabled").into(),
    "disabled" | "masked" => tr(lang, "Desabilitado", "Disabled").into(),
    _ => state.into(),
  }
}
fn log_line(e: &JournalEntry) -> Line<'static> {
  Line::from(format!(
    "{:8} {:24} {:7} {}",
    e.timestamp.as_deref().unwrap_or(""),
    e.unit.as_deref().unwrap_or("kernel"),
    e.priority_name(),
    e.message.as_deref().unwrap_or("")
  ))
}
fn run_action(user: bool, action: &str, unit: &str) -> Result<String, String> {
  let args = backend::action_args(action, unit)?;
  if !user {
    let executable = std::env::current_exe()
      .map_err(|error| error.to_string())?
      .to_string_lossy()
      .into_owned();
    let operation = SystemSettingsOperation::new(SystemProcessRunner, executable);
    let request = PrivilegedRequest::new("service", action, vec![unit.into()])?;
    let output = operation.execute(&request)?;
    return if output.status == Some(0) {
      Ok(format!("{} {}", action, unit))
    } else {
      Err(privileged_failure(&output))
    };
  }
  let mut r = ProcessRequest::new("systemctl");
  if user {
    r = r.arg("--user");
  }
  for a in &args {
    r = r.arg(a)
  }
  let out = SystemProcessRunner.run(&r).map_err(|e| e.to_string())?;
  if out.status.is_some_and(|s| s == 0) {
    Ok(format!("{} {}", action, unit))
  } else {
    Err(terminal_text(String::from_utf8_lossy(&out.stderr).trim()))
  }
}
fn privileged_failure(output: &argvus_control_center_core::process::ProcessOutput) -> String {
  let stderr = terminal_text(String::from_utf8_lossy(&output.stderr).trim());
  match output.status {
    Some(126) => "Authorization was cancelled.".into(),
    Some(127) => "Authorization was denied.".into(),
    _ if stderr.contains("pkexec was not found") => "pkexec was not found; install polkit.".into(),
    _ if stderr.is_empty() => "The privileged service operation failed.".into(),
    _ => stderr,
  }
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn failed_filter_is_explicit() {
    let mut a = ServicesApp::new(Lang::En, Theme::load());
    a.page = ServicePage::Failed;
    a.units = vec![
      Unit {
        name: "a.service".into(),
        active: "failed".into(),
        ..Default::default()
      },
      Unit {
        name: "b.service".into(),
        active: "active".into(),
        ..Default::default()
      },
    ];
    assert_eq!(a.filtered().len(), 1);
  }

  #[test]
  fn actions_are_structured_and_user_actions_are_not_elevated() {
    assert_eq!(
      backend::action_args("enable-now", "NetworkManager.service").unwrap(),
      ["enable", "--now", "NetworkManager.service"]
    );
    assert!(backend::action_args("restart", "bad unit.service").is_err());
  }

  #[test]
  fn service_list_filter_and_selection_stay_bounded() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::System;
    app.units = vec![Unit {
      name: "demo.service".into(),
      description: "Demo".into(),
      active: "active".into(),
      file_state: "enabled".into(),
      ..Default::default()
    }];
    app.handle(KeyCode::Down);
    assert_eq!(app.selected, 0);
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    app.handle(KeyCode::BackTab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Enter);
    assert_eq!(app.filter, UnitFilter::Running);
    assert_eq!(app.filtered().len(), 1);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    app.handle(KeyCode::Char('/'));
    app.handle(KeyCode::Char('z'));
    assert!(app.filtered().is_empty());
    assert_eq!(app.selected, 0);
  }

  #[test]
  fn service_details_expose_actions_and_confirmation_defaults_to_cancel() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::Detail;
    app.detail_parent = ServicePage::System;
    app.detail_unit = Some("demo.service".into());
    app.units = vec![Unit {
      name: "demo.service".into(),
      active: "active".into(),
      file_state: "enabled".into(),
      ..Default::default()
    }];
    assert!(
      app
        .detail_actions()
        .iter()
        .any(|(action, _)| *action == "restart")
    );
    app.request("restart");
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
    assert!(app.action.is_none());
    app.request("restart");
    app.handle(KeyCode::Tab);
    assert!(app.confirmation.confirm_selected);
  }

  #[test]
  fn polkit_exit_codes_are_not_collapsed_into_a_generic_error() {
    let output = argvus_control_center_core::process::ProcessOutput {
      stdout: Vec::new(),
      stderr: Vec::new(),
      status: Some(126),
      timed_out: false,
    };
    assert!(privileged_failure(&output).contains("cancelled"));
  }

  #[test]
  fn services_expose_action_buttons_on_lists_but_not_home_or_detail() {
    let home = ServicesApp::new(Lang::En, Theme::load());
    assert_eq!(home.buttons().len(), 0);
    let mut system = ServicesApp::new(Lang::En, Theme::load());
    system.page = ServicePage::System;
    assert_eq!(system.buttons().len(), 4);
    let mut logs = ServicesApp::new(Lang::En, Theme::load());
    logs.page = ServicePage::Logs;
    assert_eq!(logs.buttons().len(), 3);
  }

  #[test]
  fn services_tab_cycles_between_list_and_buttons_and_backtab_lands_last() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::System;
    app.units = (0..4)
      .map(|i| Unit {
        name: format!("svc{i}.service"),
        ..Default::default()
      })
      .collect();
    app.selected = 3;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Down);
    assert_eq!(app.selected, 3);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    assert_eq!(app.selected, 3);
  }

  #[test]
  fn services_renders_button_bar_on_list_pages() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::System;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("Start") || text.contains("Iniciar"));
    assert!(text.contains("Actions") || text.contains("Ações"));
  }

  #[test]
  fn services_home_rows_act_as_a_status_dashboard() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.units = vec![
      Unit {
        name: "a.service".into(),
        active: "active".into(),
        file_state: "enabled".into(),
        scope: "system".into(),
        ..Default::default()
      },
      Unit {
        name: "b.service".into(),
        active: "inactive".into(),
        file_state: "disabled".into(),
        scope: "system".into(),
        ..Default::default()
      },
      Unit {
        name: "c.service".into(),
        active: "failed".into(),
        file_state: "enabled".into(),
        scope: "user".into(),
        ..Default::default()
      },
    ];
    let rows = app.home_rows();
    assert_eq!(rows.len(), 4);
    assert!(rows[0].contains("System") && rows[0].contains("1 active"));
    assert!(rows[1].contains("User") && rows[1].contains("0 active"));
    assert!(rows[2].contains("Failed") && rows[2].contains("1 failed"));
    assert!(rows[3].contains("Logs"));
  }

  #[test]
  fn services_unit_rows_render_state_badges() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::System;
    app.units = vec![
      Unit {
        name: "NetworkManager.service".into(),
        active: "active".into(),
        file_state: "enabled".into(),
        ..Default::default()
      },
      Unit {
        name: "cups.service".into(),
        active: "inactive".into(),
        file_state: "disabled".into(),
        ..Default::default()
      },
      Unit {
        name: "sshd.service".into(),
        active: "failed".into(),
        file_state: "enabled".into(),
        ..Default::default()
      },
    ];
    let rows = app.unit_rows();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].starts_with("NetworkManager.service"));
    assert!(rows[0].contains("●") && rows[0].contains("Active") && rows[0].contains("Enabled"));
    assert!(rows[1].contains("○") && rows[1].contains("Inactive"));
    assert!(rows[2].contains("✕") && rows[2].contains("Failed"));
  }

  #[test]
  fn services_detail_lines_use_section_header_and_aligned_rows() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::Detail;
    app.detail_parent = ServicePage::System;
    app.detail_unit = Some("demo.service".into());
    app.units = vec![Unit {
      name: "demo.service".into(),
      description: "Demo service".into(),
      active: "active".into(),
      sub: "running".into(),
      file_state: "enabled".into(),
      main_pid: Some(1234),
      fragment: Some("/usr/lib/systemd/system/demo.service".into()),
      ..Default::default()
    }];
    let lines = app.detail_lines();
    let text = lines
      .iter()
      .map(|line| line.to_string())
      .collect::<String>();
    assert!(text.contains("SERVICE"));
    assert!(text.contains("demo.service"));
    assert!(text.contains("1234"));
    assert!(text.contains("Enabled"));
    assert!(text.contains("Unit file:"));
  }

  #[test]
  fn loading_status_is_cleared_once_units_arrive() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::System;
    app.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(app.lang, "Carregando serviços...", "Loading services...").into(),
    });
    app.job = Some(
      app
        .manager
        .spawn(|_| Ok(JobData::Units(vec![Unit::default()]))),
    );
    for _ in 0..2000 {
      if app.job.is_none() {
        break;
      }
      app.poll();
      std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(app.job.is_none());
    assert!(app.status.is_none());
    assert_eq!(app.units.len(), 1);
  }

  #[test]
  fn arrows_are_blocked_while_services_are_loading() {
    let mut app = ServicesApp::new(Lang::En, Theme::load());
    app.page = ServicePage::System;
    app.selected = 3;
    app.job = Some(app.manager.spawn(|_| {
      std::thread::sleep(std::time::Duration::from_millis(150));
      Ok(JobData::Units(vec![]))
    }));
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Up);
    app.handle(KeyCode::Home);
    app.handle(KeyCode::Tab);
    assert_eq!(app.selected, 3);
    assert!(!app.on_buttons);
  }
}
