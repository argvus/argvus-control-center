use crate::{
  backend,
  model::{LidContext, PowerBehavior, PowerButtonBehavior, PowerPage, PowerState},
};
use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::buttons::{Button, ButtonKind};
use argvus_tui::chrome::centered;
use argvus_tui::components::{
  ConfirmationDialog, ConfirmationOutcome, ConfirmationState, StatusKind, StatusMessage,
  draw_confirmation,
};
use argvus_tui::page::{list, shell, status};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

const LID_BEHAVIORS: [PowerBehavior; 5] = [
  PowerBehavior::Suspend,
  PowerBehavior::Hibernate,
  PowerBehavior::Lock,
  PowerBehavior::Ignore,
  PowerBehavior::Poweroff,
];

const BUTTON_BEHAVIORS: [PowerButtonBehavior; 4] = [
  PowerButtonBehavior::Poweroff,
  PowerButtonBehavior::Suspend,
  PowerButtonBehavior::Lock,
  PowerButtonBehavior::Ignore,
];

const IDLE_OPTIONS: [u32; 7] = [5, 10, 15, 30, 60, 120, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerTarget {
  LidBattery,
  LidAc,
  PowerButton,
  ScreenOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Picker {
  target: PickerTarget,
  selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PowerButton {
  Suspend,
  Hibernate,
  Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
  Suspend,
  Hibernate,
}

#[derive(Debug, Clone)]
enum JobData {
  State(PowerState),
  Action(String),
}

pub struct PowerApp {
  pub page: PowerPage,
  selected: usize,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  state: Option<PowerState>,
  job: Option<JobHandle<JobData>>,
  action: Option<JobHandle<JobData>>,
  pending: Option<Pending>,
  confirmation: ConfirmationState,
  picker: Option<Picker>,
  manager: JobManager,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
}

impl PowerApp {
  pub fn reload(&mut self) {
    self.refresh();
  }

  pub fn new(lang: Lang, theme: Theme) -> Self {
    let mut app = Self {
      page: PowerPage::Home,
      selected: 0,
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      state: None,
      job: None,
      action: None,
      pending: None,
      confirmation: ConfirmationState::default(),
      picker: None,
      manager: JobManager::default(),
      lang,
      theme,
      status: None,
    };
    app.refresh();
    app
  }

  fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    self.job = Some(
      self
        .manager
        .spawn(move |_| Ok(JobData::State(backend::load()))),
    );
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Carregando estado de energia...",
        "Loading power state...",
      )
      .into(),
    });
  }

  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(JobData::State(state)) => {
          self.state = Some(state);
          self.selected = self.selected.min(self.row_count().saturating_sub(1));
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
    if self.picker.is_some() {
      self.handle_picker(key);
      return false;
    }
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
      if self.page == PowerPage::Home {
        return true;
      }
      self.page = PowerPage::Home;
      self.selected = 0;
      self.on_buttons = false;
      self.button_from = None;
      self.refresh();
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
            text: tr(self.lang, "Executando...", "Running...").into(),
          });
          self.action = Some(self.manager.spawn(move |_| match pending {
            Pending::Suspend => {
              backend::suspend_now()?;
              Ok(JobData::Action(
                tr(Lang::En, "Suspensão executada", "Suspension started").into(),
              ))
            }
            Pending::Hibernate => {
              backend::hibernate_now()?;
              Ok(JobData::Action(
                tr(Lang::En, "Hibernação executada", "Hibernation started").into(),
              ))
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
      KeyCode::Up | KeyCode::Char('k') => {
        self.selected = self.selected.saturating_sub(1);
      }
      KeyCode::Down | KeyCode::Char('j') => {
        self.selected = self.selected.saturating_add(1);
      }
      KeyCode::Home => self.selected = 0,
      KeyCode::End => self.selected = self.row_count().saturating_sub(1),
      KeyCode::Enter => self.open_row_picker(),
      _ => {}
    }
    self.selected = self.selected.min(self.row_count().saturating_sub(1));
    false
  }

  fn row_count(&self) -> usize {
    let Some(state) = &self.state else { return 0; };
    if state.is_laptop {
      5
    } else {
      1
    }
  }

  fn handle_picker(&mut self, key: KeyCode) {
    let mut apply: Option<(PickerTarget, usize)> = None;
    let mut close = false;
    {
      let Some(picker) = &mut self.picker else {
        return;
      };
      let count = picker_target_len(picker.target);
      match key {
        KeyCode::Up | KeyCode::Char('k') => {
          picker.selected = picker.selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
          picker.selected = (picker.selected + 1).min(count.saturating_sub(1));
        }
        KeyCode::Home => picker.selected = 0,
        KeyCode::End => picker.selected = count.saturating_sub(1),
        KeyCode::Enter => {
          apply = Some((picker.target, picker.selected));
          close = true;
        }
        KeyCode::Esc | KeyCode::Left => close = true,
        _ => {}
      }
    }
    if close {
      self.picker = None;
    }
    if let Some((target, selected)) = apply {
      self.picker_apply(target, selected);
    }
  }

  fn open_row_picker(&mut self) {
    let Some(state) = &self.state else {
      return;
    };
    let is_laptop = state.is_laptop;

    let lid_battery_idx = 0;
    let lid_ac_idx = 1;
    let power_btn_idx = 2;
    let screen_off_idx = if is_laptop { 3 } else { 0 };

    let target = match self.selected {
      i if i == lid_battery_idx && is_laptop => PickerTarget::LidBattery,
      i if i == lid_ac_idx && is_laptop => PickerTarget::LidAc,
      i if i == power_btn_idx && is_laptop => PickerTarget::PowerButton,
      i if i == screen_off_idx => PickerTarget::ScreenOff,
      _ => return,
    };
    self.picker = Some(Picker {
      target,
      selected: picker_default(target, state),
    });
  }

  fn picker_apply(&mut self, target: PickerTarget, selected: usize) {
    match target {
      PickerTarget::LidBattery => {
        let Some(&behavior) = LID_BEHAVIORS.get(selected) else {
          return;
        };
        if let Some(state) = &mut self.state {
          state.lid[0] = behavior;
        }
        self.apply_lid(LidContext::Battery, behavior);
      }
      PickerTarget::LidAc => {
        let Some(&behavior) = LID_BEHAVIORS.get(selected) else {
          return;
        };
        if let Some(state) = &mut self.state {
          state.lid[1] = behavior;
        }
        self.apply_lid(LidContext::Ac, behavior);
      }
      PickerTarget::PowerButton => {
        let Some(&behavior) = BUTTON_BEHAVIORS.get(selected) else {
          return;
        };
        if let Some(state) = &mut self.state {
          state.power_button = behavior;
        }
        self.apply_button(behavior);
      }
      PickerTarget::ScreenOff => {
        let Some(&minutes) = IDLE_OPTIONS.get(selected) else {
          return;
        };
        if let Some(state) = &mut self.state {
          state.screen_off_minutes = (minutes > 0).then_some(minutes);
        }
        self.apply_idle(minutes);
      }
    }
  }

  fn apply_lid(&mut self, context: LidContext, behavior: PowerBehavior) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Aplicando configuração...", "Applying...").into(),
    });
    let description = format!(
      "{} · {}",
      tr(self.lang, "Fechar tampa", "Lid close"),
      behavior_label(self.lang, behavior.value()),
    );
    self.action = Some(self.manager.spawn(move |_| {
      backend::set_lid(context, behavior)?;
      Ok(JobData::Action(description))
    }));
  }

  fn apply_button(&mut self, behavior: PowerButtonBehavior) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Aplicando configuração...", "Applying...").into(),
    });
    let description = format!(
      "{} · {}",
      tr(self.lang, "Botão de energia", "Power button"),
      behavior_label(self.lang, behavior.value()),
    );
    self.action = Some(self.manager.spawn(move |_| {
      backend::set_power_button(behavior)?;
      Ok(JobData::Action(description))
    }));
  }

  fn apply_idle(&mut self, minutes: u32) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Aplicando configuração...", "Applying...").into(),
    });
    let description = format!(
      "{} · {}",
      tr(self.lang, "Desligar tela após", "Screen off after"),
      idle_label(minutes),
    );
    self.action = Some(self.manager.spawn(move |_| {
      backend::apply_idle(minutes)?;
      Ok(JobData::Action(description))
    }));
  }

  fn rows(&self) -> Vec<String> {
    let Some(state) = &self.state else {
      return vec![tr(self.lang, "Carregando...", "Loading...").into()];
    };
    let mut rows = Vec::new();

    if state.is_laptop {
      rows.push(format!(
        " {}  {}  ·  ▸ {}",
        AppConfig::icon("💻"),
        tr(
          self.lang,
          "Ao fechar a tampa (bateria)",
          "On lid close (battery)"
        ),
        behavior_label(self.lang, state.lid[0].value()),
      ));
      rows.push(format!(
        " {}  {}  ·  ▸ {}",
        AppConfig::icon("🔌"),
        tr(self.lang, "Ao fechar a tampa (CA)", "On lid close (AC)"),
        behavior_label(self.lang, state.lid[1].value()),
      ));
      rows.push(format!(
        " {}  {}  ·  ▸ {}",
        AppConfig::icon("⏻"),
        tr(self.lang, "Botão de energia", "Power button"),
        behavior_label(self.lang, state.power_button.value()),
      ));
    }

    rows.push(if state.screen_off_supported {
      format!(
        " {}  {}  ·  ▸ {}",
        AppConfig::icon("🖥️"),
        tr(self.lang, "Desligar tela após", "Screen off after"),
        idle_label(state.screen_off_minutes.unwrap_or(0)),
      )
    } else {
      format!(
        " {}  {}  ·  {}",
        AppConfig::icon("🖥️"),
        tr(self.lang, "Desligar tela após", "Screen off after"),
        tr(self.lang, "via ARGVUS hypridle", "via ARGVUS hypridle"),
      )
    });

    if state.is_laptop {
      rows.push(self.capabilities_line());
    }

    rows
  }

  fn capabilities_line(&self) -> String {
    let Some(state) = &self.state else {
      return String::new();
    };
    if !state.is_laptop {
      return String::new();
    }
    let suspend_icon = if state.can_suspend { "✓" } else { "✕" };
    let hibernate_icon = if state.can_hibernate { "✓" } else { "✕" };
    format!(
      " 🔋 {}: {}   {}: {}",
      tr(self.lang, "Suspender", "Suspend"),
      suspend_icon,
      tr(self.lang, "Hibernar", "Hibernate"),
      hibernate_icon,
    )
  }

  fn buttons(&self) -> Vec<(PowerButton, Button)> {
    let primary = ButtonKind::Primary;
    let danger = ButtonKind::Danger;
    let secondary = ButtonKind::Secondary;
    let state = self.state.as_ref();
    let mut buttons = Vec::new();
    if state.is_none_or(|s| s.can_suspend) {
      buttons.push((
        PowerButton::Suspend,
        Button::new(tr(self.lang, "Suspender agora", "Suspend now"), primary),
      ));
    }
    if state.is_none_or(|s| s.can_hibernate) {
      buttons.push((
        PowerButton::Hibernate,
        Button::new(tr(self.lang, "Hibernar agora", "Hibernate now"), danger),
      ));
    }
    buttons.push((
      PowerButton::Refresh,
      Button::new(tr(self.lang, "Atualizar", "Refresh"), secondary),
    ));
    buttons
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
    let Some((power_button, _)) = actions.get(self.button_selected) else {
      return;
    };
    match power_button {
      PowerButton::Suspend => self.pending = Some(Pending::Suspend),
      PowerButton::Hibernate => self.pending = Some(Pending::Hibernate),
      PowerButton::Refresh => self.refresh(),
    }
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let mut lines = self.rows();
    if let Some(state) = &self.state {
      if state.is_laptop {
        lines.push(self.capabilities_line());
      }
    }
    let shell_body = shell(
      frame,
      area,
      &self.theme,
      tr(self.lang, "Energia", "Power"),
      tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   Enter Lista   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   Enter List   r Refresh   ←/Esc Back   ? Help",
      ),
    );
    let buttons = self.buttons();
    let raw_buttons: Vec<Button> = buttons.into_iter().map(|(_, button)| button).collect();
    let (body, button_area) = if raw_buttons.is_empty() {
      (shell_body, None)
    } else {
      let button_height =
        argvus_tui::buttons::height(&raw_buttons, shell_body.width).min(shell_body.height);
      let split =
        Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(shell_body);
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
      let action = match pending {
        Pending::Suspend => tr(self.lang, "Suspender", "Suspend"),
        Pending::Hibernate => tr(self.lang, "Hibernar", "Hibernate"),
      };
      draw_confirmation(
        frame,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(self.lang, "Confirmar ação", "Confirm action"),
          message: &format!(
            "{} {}?",
            tr(self.lang, "Deseja", "Do you want to"),
            action.to_lowercase(),
          ),
          confirm_label: tr(self.lang, "Continuar", "Continue"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
          confirm_selected: self.confirmation.confirm_selected,
        },
      )
    }
    if let Some(picker) = &self.picker {
      draw_picker(frame, area, &self.theme, self.lang, picker);
    }
  }
}

fn behavior_label(lang: Lang, value: &str) -> String {
  match value {
    "ignore" => tr(lang, "Ignorar", "Ignore").into(),
    "poweroff" => tr(lang, "Desligar", "Power off").into(),
    "reboot" => tr(lang, "Reiniciar", "Reboot").into(),
    "halt" => tr(lang, "Parar", "Halt").into(),
    "suspend" => tr(lang, "Suspender", "Suspend").into(),
    "hibernate" => tr(lang, "Hibernar", "Hibernate").into(),
    "suspend-then-hibernate" => tr(lang, "Suspender → hibernar", "Suspend → hibernate").into(),
    "lock" => tr(lang, "Bloquear", "Lock").into(),
    other => other.into(),
  }
}

fn idle_label(minutes: u32) -> String {
  if minutes == 0 {
    String::from("Nunca")
  } else if minutes < 60 {
    format!("{minutes} min")
  } else {
    format!("{} h", minutes / 60)
  }
}

fn picker_target_len(target: PickerTarget) -> usize {
  match target {
    PickerTarget::LidBattery | PickerTarget::LidAc => LID_BEHAVIORS.len(),
    PickerTarget::PowerButton => BUTTON_BEHAVIORS.len(),
    PickerTarget::ScreenOff => IDLE_OPTIONS.len(),
  }
}

/// The current value's position within the picker option list.
fn picker_default(target: PickerTarget, state: &PowerState) -> usize {
  match target {
    PickerTarget::LidBattery => LID_BEHAVIORS
      .iter()
      .position(|&value| value == state.lid[0])
      .unwrap_or(0),
    PickerTarget::LidAc => LID_BEHAVIORS
      .iter()
      .position(|&value| value == state.lid[1])
      .unwrap_or(0),
    PickerTarget::PowerButton => BUTTON_BEHAVIORS
      .iter()
      .position(|&value| value == state.power_button)
      .unwrap_or(0),
    PickerTarget::ScreenOff => {
      let current = state.screen_off_minutes.unwrap_or(0);
      IDLE_OPTIONS
        .iter()
        .position(|&value| value == current)
        .unwrap_or_else(|| IDLE_OPTIONS.len().saturating_sub(1))
    }
  }
}

fn picker_options(target: PickerTarget, lang: Lang) -> Vec<String> {
  match target {
    PickerTarget::LidBattery | PickerTarget::LidAc => LID_BEHAVIORS
      .iter()
      .map(|&behavior| behavior_label(lang, behavior.value()))
      .collect(),
    PickerTarget::PowerButton => BUTTON_BEHAVIORS
      .iter()
      .map(|&behavior| behavior_label(lang, behavior.value()))
      .collect(),
    PickerTarget::ScreenOff => IDLE_OPTIONS.iter().map(|&minutes| idle_label(minutes)).collect(),
  }
}

fn picker_title(lang: Lang, target: PickerTarget) -> String {
  match target {
    PickerTarget::LidBattery => tr(lang, "Tampa (bateria)", "Lid close (battery)").into(),
    PickerTarget::LidAc => tr(lang, "Tampa (CA)", "Lid close (AC)").into(),
    PickerTarget::PowerButton => tr(lang, "Botão de energia", "Power button").into(),
    PickerTarget::ScreenOff => tr(lang, "Desligar tela após", "Screen off after").into(),
  }
}

fn draw_picker(
  frame: &mut Frame,
  area: Rect,
  theme: &Theme,
  lang: Lang,
  picker: &Picker,
) {
  let options = picker_options(picker.target, lang);
  let title = picker_title(lang, picker.target);
  let width = area.width.saturating_sub(8).clamp(28, 48);
  let height = (options.len() as u16).saturating_add(2).min(area.height);
  let popup = centered(area, width, height.max(4));
  frame.render_widget(Clear, popup);
  let lines: Vec<Line> = options
    .iter()
    .enumerate()
    .map(|(index, option)| {
      let chosen = index == picker.selected;
      let style = if chosen {
        Style::new()
          .fg(theme.selected_foreground)
          .bg(theme.selected_background)
          .add_modifier(Modifier::BOLD)
      } else {
        Style::new().fg(theme.foreground)
      };
      Line::from(Span::styled(
        format!(" {} {} ", if chosen { ">" } else { " " }, option),
        style,
      ))
    })
    .collect();
  frame.render_widget(
    Paragraph::new(lines)
      .block(
        Block::bordered()
          .title(format!(" {title} "))
          .border_style(Style::new().fg(theme.border_active))
          .style(Style::new().bg(theme.background)),
      ),
    popup,
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  fn laptop_state() -> PowerState {
    PowerState {
      lid: [PowerBehavior::Suspend, PowerBehavior::Hibernate],
      power_button: PowerButtonBehavior::Poweroff,
      screen_off_minutes: Some(15),
      can_suspend: true,
      can_hibernate: true,
      screen_off_supported: true,
      is_laptop: true,
    }
  }

  fn desktop_state() -> PowerState {
    PowerState {
      lid: [PowerBehavior::Suspend, PowerBehavior::Hibernate],
      power_button: PowerButtonBehavior::Poweroff,
      screen_off_minutes: Some(15),
      can_suspend: true,
      can_hibernate: true,
      screen_off_supported: true,
      is_laptop: false,
    }
  }

  #[test]
  fn energy_app_loads_in_the_background() {
    let app = PowerApp::new(Lang::En, Theme::load());
    assert!(app.job.is_some());
    assert!(app.status.is_some());
  }

  #[test]
  fn row_count_laptop() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    assert_eq!(app.row_count(), 0);
    app.state = Some(laptop_state());
    assert_eq!(app.row_count(), 5);
  }

  #[test]
  fn row_count_desktop() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    assert_eq!(app.row_count(), 0);
    app.state = Some(desktop_state());
    assert_eq!(app.row_count(), 1);
  }

  #[test]
  fn picker_opens_closes_and_applies_screen_off() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.job = None;
    app.action = None;
    app.state = Some(desktop_state());
    app.selected = 0;
    app.handle(KeyCode::Enter);
    assert!(app.picker.is_some());
    assert_eq!(app.picker.unwrap().target, PickerTarget::ScreenOff);
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Enter);
    assert!(app.picker.is_none());
    assert_eq!(app.state.as_ref().unwrap().screen_off_minutes, Some(60));
    assert!(app.action.is_some());
  }

  #[test]
  fn picker_escape_cancels_without_change() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.job = None;
    app.action = None;
    app.state = Some(desktop_state());
    app.selected = 0;
    app.handle(KeyCode::Enter);
    app.handle(KeyCode::Down);
    assert_eq!(app.picker.unwrap().selected, 3);
    app.handle(KeyCode::Esc);
    assert!(app.picker.is_none());
    assert_eq!(app.state.as_ref().unwrap().screen_off_minutes, Some(15));
    assert!(app.action.is_none());
  }

  #[test]
  fn picker_nunca_clears_screen_off() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.job = None;
    app.action = None;
    app.state = Some(desktop_state());
    app.selected = 0;
    app.handle(KeyCode::Enter);
    app.handle(KeyCode::End);
    assert_eq!(app.picker.unwrap().selected, 6);
    app.handle(KeyCode::Enter);
    assert!(app.picker.is_none());
    assert_eq!(app.state.as_ref().unwrap().screen_off_minutes, None);
    assert!(app.action.is_some());
  }

  #[test]
  fn picker_navigation_is_bounded() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.job = None;
    app.action = None;
    app.state = Some(desktop_state());
    app.selected = 0;
    app.handle(KeyCode::Enter);
    for _ in 0..6 {
      app.handle(KeyCode::Down);
    }
    assert_eq!(app.picker.unwrap().selected, 6);
    app.handle(KeyCode::Home);
    assert_eq!(app.picker.unwrap().selected, 0);
  }

  #[test]
  fn lid_picker_applies_selected_behavior() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.job = None;
    app.action = None;
    app.state = Some(laptop_state());
    app.selected = 0;
    app.handle(KeyCode::Enter);
    assert_eq!(app.picker.unwrap().target, PickerTarget::LidBattery);
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Enter);
    assert!(app.picker.is_none());
    assert_eq!(app.state.as_ref().unwrap().lid[0], PowerBehavior::Hibernate);
    assert!(app.action.is_some());
  }

  #[test]
  fn picker_defaults_match_current_state() {
    let state = laptop_state();
    assert_eq!(picker_default(PickerTarget::LidBattery, &state), 0);
    assert_eq!(picker_default(PickerTarget::LidAc, &state), 1);
    assert_eq!(picker_default(PickerTarget::PowerButton, &state), 0);
    assert_eq!(picker_target_len(PickerTarget::ScreenOff), 7);
    let desktop = desktop_state();
    assert_eq!(picker_default(PickerTarget::ScreenOff, &desktop), 2);
  }

  #[test]
  fn power_app_navigates_and_toggles_button_focus() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.job = None;
    app.action = None;
    app.state = Some(laptop_state());
    app.handle(KeyCode::Down);
    assert_eq!(app.selected, 1);
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    app.handle(KeyCode::BackTab);
    assert!(app.on_buttons);
  }

  #[test]
  fn idle_label_handles_hours_and_minutes() {
    assert_eq!(idle_label(0), "Nunca");
    assert_eq!(idle_label(15), "15 min");
    assert_eq!(idle_label(60), "1 h");
    assert_eq!(idle_label(120), "2 h");
  }

  #[test]
  fn power_app_renders_without_panic() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.state = Some(laptop_state());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
  }

  #[test]
  fn power_app_desktop_renders_without_panic() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.state = Some(desktop_state());
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
  }

  #[test]
  fn power_app_renders_with_picker_open() {
    let mut app = PowerApp::new(Lang::En, Theme::load());
    app.state = Some(desktop_state());
    app.picker = Some(Picker {
      target: PickerTarget::ScreenOff,
      selected: 6,
    });
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
  }
}