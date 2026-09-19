//! Implements terminal UI rendering and interaction in crate `argvus control center displays`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use crate::{
  backend,
  model::{
    DisplayPage, Monitor, MonitorProfile, MonitorSetting, PersistedConfig, PersistedMonitor,
    PromptGoal,
  },
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
};
use argvus_tui::page::{list, readonly, shell, status};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use std::time::{Duration, Instant};

/// Defines the constant `REVERT_SECONDS`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const REVERT_SECONDS: u64 = 15;
/// Defines the constant `HOTPLUG_INTERVAL`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const HOTPLUG_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, PartialEq)]
/// Defines `PersistChange`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
enum PersistChange {
  Mode(String),
  Scale(f64),
  Position(i32, i32),
  Vrr(i32),
  Hdr(i32),
  Mirror(String),
  BitDepth(i32),
  Disabled(bool),
  Workspace(u32, Option<String>),
  Dpms(bool),
  Primary,
  None,
}

#[derive(Debug, Clone, PartialEq)]
/// Represents `PickerOption`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
struct PickerOption {
  label: String,
  args: String,
  persist: PersistChange,
  /// When set, selecting this row opens the free-form editor goal instead.
  prompt: Option<PromptGoal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Defines `DisplayButton`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
enum DisplayButton {
  Refresh,
  Apply,
  Reset,
  Remove,
  Profiles,
  ProfileNew,
  ProfileApply,
  ProfileRename,
  ProfileDelete,
}

#[derive(Debug, Clone)]
/// Defines `JobData`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
enum JobData {
  Snapshot {
    monitors: Vec<Monitor>,
    config: PersistedConfig,
    state: crate::model::DisplayState,
    version: (u32, u32),
  },
  Hotplug(Vec<Monitor>),
  Action(String),
}

/// Represents `RevertState`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
struct RevertState {
  name: String,
  previous: Monitor,
  previous_config: PersistedConfig,
  deadline: Instant,
}

/// Defines `HomeEntry`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
enum HomeEntry {
  Live(usize),
  Stale {
    name: String,
    persisted: PersistedMonitor,
  },
  Profiles,
}

/// Represents `DisplaysApp`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct DisplaysApp {
  pub page: DisplayPage,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
  monitors: Vec<Monitor>,
  config: PersistedConfig,
  state: crate::model::DisplayState,
  version: (u32, u32),
  selected: usize,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  job: Option<JobHandle<JobData>>,
  action: Option<JobHandle<JobData>>,
  hotplug: Option<JobHandle<JobData>>,
  last_hotplug: Instant,
  revert: Option<RevertState>,
  confirm_profile: Option<(usize, ConfirmationState)>,
  prompt_buffer: String,
  prompt_error: Option<String>,
  prompt_back: Option<DisplayPage>,
  manager: JobManager,
}

impl DisplaysApp {
  /// Executes the `reload` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn reload(&mut self) {
    self.refresh();
  }

  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn new(lang: Lang, theme: Theme) -> Self {
    let mut app = Self {
      page: DisplayPage::Home,
      lang,
      theme,
      status: None,
      monitors: vec![],
      config: PersistedConfig::default(),
      state: crate::model::DisplayState::default(),
      version: (0, 0),
      selected: 0,
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      job: None,
      action: None,
      hotplug: None,
      last_hotplug: Instant::now()
        .checked_sub(HOTPLUG_INTERVAL * 6)
        .unwrap_or(Instant::now()),
      revert: None,
      confirm_profile: None,
      prompt_buffer: String::new(),
      prompt_error: None,
      prompt_back: None,
      manager: JobManager::default(),
    };
    app.refresh();
    app
  }

  /// Executes the `refresh` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    let lang = self.lang;
    self.job = Some(self.manager.spawn(move |_| {
      let version = backend::hyprctl_version();
      let monitors = backend::list_monitors().unwrap_or_default();
      let config = backend::load_config();
      let state = backend::load_state();
      Ok(JobData::Snapshot {
        monitors,
        config,
        state,
        version,
      })
    }));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(lang, "control_center.loading_monitors").into(),
    });
  }

  /// Executes the `poll` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(JobData::Snapshot {
          monitors,
          config,
          state,
          version,
        }) => {
          self.monitors = monitors;
          self.config = config;
          self.state = state;
          self.version = version;
          self.selected = self.selected.min(self.selection_len().saturating_sub(1));
          self.status = None;
        }
        Ok(JobData::Hotplug(_)) | Ok(JobData::Action(_)) => {}
        Err(e) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: e,
          })
        }
      };
      self.last_hotplug = Instant::now();
      changed = true;
    }
    if let Some(action_job) = &self.action
      && let JobState::Finished(result) = action_job.try_state()
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
    if let Some(revert) = self.revert.take_if(|r| Instant::now() >= r.deadline) {
      let previous_config = revert.previous_config;
      self.spawn_revert(
        previous_config,
        format!(
          "{} {}",
          tr(self.lang, "control_center.reverted"),
          revert.previous.name
        ),
      );
      changed = true;
    }
    if let Some(hotplug) = &self.hotplug
      && let JobState::Finished(result) = hotplug.try_state()
    {
      self.hotplug = None;
      if let Ok(JobData::Hotplug(monitors)) = result {
        let changed_monitors = fingerprint(&self.monitors) != fingerprint(&monitors);
        if changed_monitors {
          self.monitors = monitors;
          self.selected = self.selected.min(self.selection_len().saturating_sub(1));
          if let DisplayPage::Detail(index) = self.page
            && index >= self.monitors.len()
          {
            self.page = DisplayPage::Home;
            self.selected = 0;
            self.on_buttons = false;
          }
          self.status = Some(StatusMessage {
            kind: StatusKind::Info,
            text: tr(self.lang, "control_center.monitors_changed_hotplug").into(),
          });
          changed = true;
        }
      }
      self.last_hotplug = Instant::now();
    }
    if self.job.is_none()
      && self.action.is_none()
      && self.hotplug.is_none()
      && self.revert.is_none()
      && !matches!(self.page, DisplayPage::Prompt { .. })
      && self.last_hotplug.elapsed() >= HOTPLUG_INTERVAL
    {
      self.hotplug = Some(self.manager.spawn(|_| {
        let monitors = backend::list_monitors().unwrap_or_default();
        Ok(JobData::Hotplug(monitors))
      }));
      self.last_hotplug = Instant::now();
    }
    changed
  }

  /// Processes `handle` in this module's event flow. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if let Some((index, mut confirm)) = self.confirm_profile.take() {
      match confirm.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.delete_profile(index);
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: tr(self.lang, "control_center.profile_deleted").into(),
          });
          self.selected = self
            .selected
            .min(self.state.profiles.len().saturating_sub(1));
        }
        ConfirmationOutcome::Cancelled => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Info,
            text: tr(self.lang, "control_center.deletion_cancelled").into(),
          });
        }
        ConfirmationOutcome::Pending => {
          self.confirm_profile = Some((index, confirm));
        }
      }
      return false;
    }
    if self.revert.is_some() {
      match key {
        KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('y') | KeyCode::Char('Y') => {
          let name = self.revert.take().map(|revert| revert.name);
          if let Some(name) = name {
            self.status = Some(StatusMessage {
              kind: StatusKind::Success,
              text: format!(
                "{} · {name}",
                tr(self.lang, "control_center.configuration_kept")
              ),
            });
          }
          return false;
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
          let revert = self.revert.take();
          if let Some(revert) = revert {
            self.spawn_revert(
              revert.previous_config,
              format!(
                "{} {name}",
                tr(self.lang, "control_center.reverted"),
                name = revert.previous.name
              ),
            );
          }
          return false;
        }
        _ => return false,
      }
    }
    if self.prompt_back.is_some() {
      return self.prompt_key(key);
    }
    if self.on_buttons && !self.buttons().is_empty() {
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
    if matches!(key, KeyCode::Esc | KeyCode::Left) {
      match self.page {
        DisplayPage::Home => return true,
        DisplayPage::Profiles => {
          self.page = DisplayPage::Home;
          self.selected = 0;
        }
        DisplayPage::Picker { monitor, .. } => {
          self.page = DisplayPage::Detail(monitor);
          self.selected = 0;
        }
        DisplayPage::Detail(_) => {
          self.page = DisplayPage::Home;
          self.selected = 0;
        }
        DisplayPage::Prompt { goal } => self.restore_prompt(goal),
      }
      self.on_buttons = false;
      self.button_from = None;
      return false;
    }
    if self.job.is_some() || self.action.is_some() {
      return false;
    }
    match self.page {
      DisplayPage::Home => match key {
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
      },
      DisplayPage::Detail(_) => match key {
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
      },
      DisplayPage::Profiles => match key {
        KeyCode::Tab | KeyCode::BackTab => self.toggle_buttons(key == KeyCode::BackTab),
        KeyCode::Char('r') => self.reload_state(),
        KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => self.selected = self.selected.saturating_add(1),
        KeyCode::Home => self.selected = 0,
        KeyCode::End => self.selected = self.selection_len().saturating_sub(1),
        KeyCode::Enter if self.selected == self.state.profiles.len() => {
          self.open_prompt(PromptGoal::ProfileCreate)
        }
        _ => {}
      },
      DisplayPage::Picker { .. } => match key {
        KeyCode::Char('r') | KeyCode::Char('q') => self.exit_picker(),
        KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => self.selected = self.selected.saturating_add(1),
        KeyCode::Home => self.selected = 0,
        KeyCode::End => self.selected = self.selection_len().saturating_sub(1),
        KeyCode::Enter => self.apply_picker_selection(),
        _ => {}
      },
      DisplayPage::Prompt { goal } => {
        let _ = goal;
      }
    }
    self.selected = self.selected.min(self.selection_len().saturating_sub(1));
    false
  }

  /// Executes the `prompt_key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn prompt_key(&mut self, key: KeyCode) -> bool {
    let DisplayPage::Prompt { goal } = self.page else {
      return false;
    };
    match key {
      KeyCode::Esc => {
        self.restore_prompt(goal);
        false
      }
      KeyCode::Backspace => {
        self.prompt_buffer.pop();
        false
      }
      KeyCode::Char(c) if prompt_allowed(goal, c) => {
        self.prompt_buffer.push(c);
        false
      }
      KeyCode::Enter => {
        self.commit_prompt(goal);
        false
      }
      _ => false,
    }
  }

  /// Executes the `restore_prompt` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn restore_prompt(&mut self, goal: PromptGoal) {
    self.prompt_buffer.clear();
    self.prompt_error = None;
    self.page = self.prompt_back.take().unwrap_or(match goal {
      PromptGoal::Position(i)
      | PromptGoal::Scale(i)
      | PromptGoal::SdrBrightness(i)
      | PromptGoal::SdrSaturation(i) => DisplayPage::Detail(i),
      PromptGoal::ProfileCreate | PromptGoal::ProfileRename(_) => DisplayPage::Profiles,
    });
    self.selected = 0;
    self.on_buttons = false;
    self.button_from = None;
  }

  /// Executes the `commit_prompt` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn commit_prompt(&mut self, goal: PromptGoal) {
    let buffer = self.prompt_buffer.clone();
    match goal {
      PromptGoal::Position(index) => {
        let Some((x, y)) = parse_position(&buffer) else {
          self.prompt_error = Some(
            tr(
              self.lang,
              "control_center.invalid_position_use_x_y_e_g_1920_0",
            )
            .into(),
          );
          return;
        };
        let Some(monitor) = self.monitors.get(index).cloned() else {
          return;
        };
        let name = monitor.name.clone();
        let mut config = self.config.clone();
        let mut persisted = config.persisted(&name);
        persisted.position = Some(format!("{x}x{y}"));
        config.set_monitor(&name, persisted);
        let prev = self.config.clone();
        self.config = config.clone();
        self.revert = Some(RevertState {
          name: name.clone(),
          previous: monitor.clone(),
          previous_config: prev,
          deadline: Instant::now() + Duration::from_secs(REVERT_SECONDS),
        });
        let message = format!(
          "{} · {name} ({x}, {y})",
          tr(self.lang, "control_center.position")
        );
        self.page = DisplayPage::Detail(index);
        self.selected = 0;
        self.on_buttons = false;
        self.spawn_apply_with_config(config, message);
      }
      PromptGoal::Scale(index) => {
        let Some(value) = parse_number(&buffer) else {
          self.prompt_error = Some(
            tr(
              self.lang,
              "control_center.invalid_value_use_a_number_e_g_1_25",
            )
            .into(),
          );
          return;
        };
        let Some(monitor) = self.monitors.get(index).cloned() else {
          return;
        };
        let name = monitor.name.clone();
        let mut config = self.config.clone();
        let mut persisted = config.persisted(&name);
        persisted.scale = Some(value);
        config.set_monitor(&name, persisted);
        self.config = config.clone();
        let message = format!(
          "{} · {name}: {value}",
          tr(self.lang, "control_center.scale")
        );
        self.page = DisplayPage::Detail(index);
        self.selected = 0;
        self.on_buttons = false;
        self.spawn_apply_with_config(config, message);
      }
      PromptGoal::SdrBrightness(index) => {
        let Some(value) = parse_number(&buffer) else {
          self.prompt_error =
            Some(tr(self.lang, "control_center.invalid_value_use_0_0_to_1_0").into());
          return;
        };
        self.apply_sdr(index, |persisted| {
          persisted.sdr_brightness = Some(value.clamp(0.0, 1.0))
        });
      }
      PromptGoal::SdrSaturation(index) => {
        let Some(value) = parse_number(&buffer) else {
          self.prompt_error =
            Some(tr(self.lang, "control_center.invalid_value_use_0_0_to_2_0").into());
          return;
        };
        self.apply_sdr(index, |saturation| {
          saturation.sdr_saturation = Some(value.clamp(0.0, 2.0))
        });
      }
      PromptGoal::ProfileCreate => {
        if buffer.trim().is_empty() {
          self.prompt_error =
            Some(tr(self.lang, "control_center.profile_name_cannot_be_empty").into());
          return;
        }
        if self.state.profile_index(buffer.trim()).is_some() {
          self.prompt_error = Some(
            tr(
              self.lang,
              "control_center.a_profile_with_this_name_already_exists",
            )
            .into(),
          );
          return;
        }
        let profile = MonitorProfile {
          name: buffer.trim().to_string(),
          apply_wallpapers: false,
          config: self.config.clone(),
        };
        self.state.profiles.push(profile);
        self.state.active_profile = None;
        let state = self.state.clone();
        self.spawn_save_state(
          state,
          format!(
            "{} · {}",
            tr(self.lang, "control_center.profile_created"),
            buffer.trim()
          ),
        );
        self.page = DisplayPage::Profiles;
        self.selected = self.state.profiles.len().saturating_sub(1);
        self.prompt_buffer.clear();
        self.prompt_error = None;
        self.prompt_back = None;
        self.on_buttons = false;
      }
      PromptGoal::ProfileRename(index) => {
        if buffer.trim().is_empty() {
          self.prompt_error =
            Some(tr(self.lang, "control_center.profile_name_cannot_be_empty").into());
          return;
        }
        if let Some(profile) = self.state.profiles.get_mut(index) {
          profile.name = buffer.trim().to_string();
        }
        let state = self.state.clone();
        self.spawn_save_state(
          state,
          format!(
            "{} · {}",
            tr(self.lang, "control_center.profile_renamed"),
            buffer.trim()
          ),
        );
        self.page = DisplayPage::Profiles;
        self.selected = index;
        self.prompt_buffer.clear();
        self.prompt_error = None;
        self.prompt_back = None;
        self.on_buttons = false;
      }
    }
  }

  /// Applies the `apply_sdr` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_sdr<F: FnOnce(&mut PersistedMonitor)>(&mut self, index: usize, mutate: F) {
    let Some(monitor) = self.monitors.get(index).cloned() else {
      return;
    };
    let name = monitor.name.clone();
    let mut config = self.config.clone();
    let mut persisted = config.persisted(&name);
    mutate(&mut persisted);
    config.set_monitor(&name, persisted.clone());
    self.config = config.clone();
    let message = format!("{} · {name}", tr(self.lang, "control_center.sdr"));
    self.page = DisplayPage::Detail(index);
    self.selected = 0;
    self.on_buttons = false;
    self.spawn_apply_with_config(config, message);
  }

  /// Executes the `spawn_save_state` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn spawn_save_state(&mut self, state: crate::model::DisplayState, message: String) {
    self.action = Some(self.manager.spawn(move |_| {
      backend::save_state(&state)?;
      Ok(JobData::Action(message))
    }));
  }

  /// Executes the `exit_picker` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn exit_picker(&mut self) {
    let page = self.page;
    if let DisplayPage::Picker { monitor, .. } = page {
      self.page = DisplayPage::Detail(monitor);
    }
    self.selected = 0;
    self.on_buttons = false;
  }

  /// Executes the `selection_len` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn selection_len(&self) -> usize {
    match self.page {
      DisplayPage::Home => self.home_entries().len(),
      DisplayPage::Detail(_) => self.detail_settings().len(),
      DisplayPage::Profiles => self.profile_rows().len(),
      DisplayPage::Picker { .. } => self.picker_options().len(),
      DisplayPage::Prompt { .. } => 0,
    }
  }

  /// Executes the `open` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn open(&mut self) {
    match self.page {
      DisplayPage::Home => match self.home_entries().get(self.selected) {
        Some(HomeEntry::Live(index)) => {
          self.page = DisplayPage::Detail(*index);
          self.selected = 0;
          self.on_buttons = false;
        }
        Some(HomeEntry::Stale { .. }) => {
          self.page = DisplayPage::Detail(self.stale_start());
          self.selected = 0;
          self.on_buttons = false;
        }
        Some(HomeEntry::Profiles) => {
          self.page = DisplayPage::Profiles;
          self.selected = 0;
          self.on_buttons = false;
        }
        None => {}
      },
      DisplayPage::Detail(index) => {
        if let Some(setting) = self.detail_setting(self.selected) {
          self.page = DisplayPage::Picker {
            monitor: index,
            setting,
          };
          self.selected = 0;
          self.on_buttons = false;
        }
      }
      DisplayPage::Picker { .. } => self.apply_picker_selection(),
      DisplayPage::Profiles => {
        if self.selected == self.state.profiles.len() {
          self.open_prompt(PromptGoal::ProfileCreate);
        }
      }
      DisplayPage::Prompt { .. } => {}
    }
  }

  /// Executes the `stale_start` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn stale_start(&self) -> usize {
    self.monitors.len()
  }

  /// Executes the `stale_name` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn stale_name(&self, index: usize) -> Option<String> {
    let start = self.stale_start();
    if index < start {
      return None;
    }
    let stale: Vec<&String> = self
      .config
      .monitors
      .iter()
      .filter(|(name, _)| !self.monitors.iter().any(|monitor| &monitor.name == name))
      .map(|(name, _)| name)
      .collect();
    stale.get(index - start).map(|name| name.to_string())
  }

  /// Executes the `detail_setting` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn detail_setting(&self, row: usize) -> Option<MonitorSetting> {
    let settings = self.detail_settings();
    settings.get(row).copied()
  }

  /// Executes the `detail_settings` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn detail_settings(&self) -> Vec<MonitorSetting> {
    let index = match self.monitor_index() {
      Some(index) => index,
      None => return vec![],
    };
    let Some(monitor) = self.monitors.get(index) else {
      return vec![];
    };
    if !monitor.connected {
      return vec![];
    }
    let mut settings = vec![
      MonitorSetting::Resolution,
      MonitorSetting::RefreshRate,
      MonitorSetting::Scale,
      MonitorSetting::Position,
      MonitorSetting::Orientation,
      MonitorSetting::Enabled,
      MonitorSetting::Mirror,
      MonitorSetting::BitDepth,
    ];
    if self.vrr_supported() {
      settings.push(MonitorSetting::Vrr);
    }
    if self.hdr_supported() && monitor.has_10bit() {
      settings.push(MonitorSetting::Hdr);
    }
    settings.push(MonitorSetting::Dpms);
    if self.hdr_supported() && monitor.has_10bit() {
      settings.push(MonitorSetting::SdrBrightness);
      settings.push(MonitorSetting::SdrSaturation);
    }
    settings.push(MonitorSetting::Workspaces);
    settings.push(MonitorSetting::Primary);
    settings
  }

  /// Executes the `monitor_index` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn monitor_index(&self) -> Option<usize> {
    match self.page {
      DisplayPage::Detail(index) | DisplayPage::Picker { monitor: index, .. } => Some(index),
      DisplayPage::Home | DisplayPage::Profiles | DisplayPage::Prompt { .. } => None,
    }
  }

  /// Executes the `detail_info_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn detail_info_rows(&self) -> Vec<(String, String)> {
    let Some(index) = self.monitor_index() else {
      return vec![];
    };
    let Some(monitor) = self.monitors.get(index) else {
      return vec![];
    };
    monitor.info_rows()
  }

  /// Executes the `detail_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn detail_rows(&self) -> Vec<String> {
    let Some(index) = self.monitor_index() else {
      return vec![];
    };
    let Some(monitor) = self.monitors.get(index) else {
      let name = self.stale_name(index).unwrap_or_default();
      return vec![format!(
        " {} {} {name}",
        AppConfig::icon(argvus_tui::icons::ETHERNET),
        tr(self.lang, "control_center.monitor_disconnected")
      )];
    };
    if !monitor.connected {
      return vec![format!(
        " {} {}",
        AppConfig::icon(argvus_tui::icons::ETHERNET),
        tr(self.lang, "control_center.monitor_disconnected")
      )];
    }
    let persisted = self.config.persisted(&monitor.name);
    let mode = persisted
      .mode
      .clone()
      .unwrap_or_else(|| format!("{}x{}", monitor.width, monitor.height));
    let rate = persisted
      .mode
      .as_deref()
      .and_then(rate_of)
      .unwrap_or(monitor.refresh_rate);
    let scale = persisted.scale.unwrap_or(monitor.scale);
    let position = persisted
      .position
      .as_deref()
      .map(|value| value.to_string())
      .unwrap_or(format!("{}x{}", monitor.x, monitor.y));
    let transform = persisted.transform.unwrap_or(monitor.transform);
    let vrr = persisted.vrr.unwrap_or(monitor.vrr);
    let hdr = persisted.hdr.unwrap_or(0);
    let primary = self.config.primary_monitor.as_deref() == Some(monitor.name.as_str());
    let mirror = persisted
      .mirror
      .clone()
      .or_else(|| monitor.info.mirror_of.clone())
      .unwrap_or_else(|| tr(self.lang, "control_center.none").into());
    let mut rows = vec![
      format!(
        " {}  {}  ·  {mode}",
        tr(self.lang, "control_center.resolution"),
        monitor.name
      ),
      format!(
        " {}  ·  {:.3} Hz",
        tr(self.lang, "control_center.refresh_rate"),
        rate
      ),
      format!(" {}  ·  {}", tr(self.lang, "control_center.scale"), scale),
      format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.position"),
        position
      ),
      format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.orientation"),
        transform_label(self.lang, transform)
      ),
      format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.monitor_enabled"),
        on_off(self.lang, persisted.disabled != Some(true))
      ),
      format!(" {}  ·  {mirror}", tr(self.lang, "control_center.mirror")),
      format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.color_depth"),
        self.bitdepth_label(monitor, &persisted)
      ),
    ];
    if self.vrr_supported() {
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.vrr"),
        vrr_label(self.lang, vrr)
      ));
    }
    if self.hdr_supported() && monitor.has_10bit() {
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.hdr"),
        hdr_label(self.lang, hdr)
      ));
    }
    rows.push(format!(
      " {}  ·  {}",
      tr(self.lang, "control_center.dpms"),
      if monitor.dpms_status == "off" {
        tr(self.lang, "control_center.off")
      } else {
        tr(self.lang, "control_center.on")
      }
    ));
    if self.hdr_supported() && monitor.has_10bit() {
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.sdr_brightness"),
        persisted
          .sdr_brightness
          .or(monitor.info.sdr_brightness)
          .map(|value| format!("{value:.2}"))
          .unwrap_or_else(|| tr(self.lang, "control_center.auto").into())
      ));
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "control_center.sdr_saturation"),
        persisted
          .sdr_saturation
          .or(monitor.info.sdr_saturation)
          .map(|value| format!("{value:.2}"))
          .unwrap_or_else(|| tr(self.lang, "control_center.auto").into())
      ));
    }
    let workspaces = self.config.workspaces_of(&monitor.name);
    rows.push(format!(
      " {}  ·  {}",
      tr(self.lang, "control_center.workspaces"),
      if workspaces.is_empty() {
        tr(self.lang, "control_center.unbound").into()
      } else {
        workspaces
          .iter()
          .map(|value| value.to_string())
          .collect::<Vec<_>>()
          .join(", ")
      }
    ));
    rows.push(format!(
      " {}  ·  {}",
      tr(self.lang, "control_center.primary_monitor"),
      on_off(self.lang, primary)
    ));
    rows
  }

  /// Executes the `bitdepth_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn bitdepth_label(&self, monitor: &Monitor, persisted: &PersistedMonitor) -> String {
    if !monitor.has_10bit() {
      return tr(self.lang, "control_center.unsupported").into();
    }
    match persisted.bitdepth {
      Some(10) => "10".into(),
      Some(8) => "8".into(),
      _ if monitor.info.current_format.contains("10") => "10".into(),
      _ => "8".into(),
    }
  }

  /// Executes the `picker_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn picker_options(&self) -> Vec<PickerOption> {
    let Some((monitor_index, setting)) = self.monitor_ctx() else {
      return vec![];
    };
    let Some(monitor) = self.monitors.get(monitor_index) else {
      return vec![];
    };
    match setting {
      MonitorSetting::Resolution => resolution_options(monitor),
      MonitorSetting::RefreshRate => refresh_options(monitor),
      MonitorSetting::Scale => {
        let mut options = scale_options(monitor);
        options.push(PickerOption {
          label: format!("✎ {}", tr(self.lang, "control_center.custom")),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::Scale(monitor_index)),
        });
        options
      }
      MonitorSetting::Position => {
        let mut options = self.position_options(monitor_index);
        options.push(PickerOption {
          label: format!("✎ {}", tr(self.lang, "control_center.custom")),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::Position(monitor_index)),
        });
        options
      }
      MonitorSetting::Primary => self.primary_options(),
      MonitorSetting::Enabled => enabled_options(monitor, self.lang),
      MonitorSetting::Mirror => mirror_options(monitor, &self.monitors, self.lang),
      MonitorSetting::BitDepth if monitor.has_10bit() => bitdepth_options(monitor, self.lang),
      MonitorSetting::Vrr if self.vrr_supported() => vrr_options(monitor, self.lang),
      MonitorSetting::Hdr if self.hdr_supported() && monitor.has_10bit() => {
        hdr_options(monitor, self.lang)
      }
      MonitorSetting::Dpms => dpms_options(monitor, self.lang),
      MonitorSetting::Orientation => orientation_options(monitor, self.lang),
      MonitorSetting::SdrBrightness if self.hdr_supported() && monitor.has_10bit() => {
        let mut options = sdr_brightness_options(self.lang, monitor);
        options.push(PickerOption {
          label: format!("✎ {}", tr(self.lang, "control_center.custom")),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::SdrBrightness(monitor_index)),
        });
        options
      }
      MonitorSetting::SdrSaturation if self.hdr_supported() && monitor.has_10bit() => {
        let mut options = sdr_saturation_options(self.lang, monitor);
        options.push(PickerOption {
          label: format!("✎ {}", tr(self.lang, "control_center.custom")),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::SdrSaturation(monitor_index)),
        });
        options
      }
      MonitorSetting::Workspaces => self.workspace_options(monitor_index),
      MonitorSetting::BitDepth => vec![PickerOption {
        label: tr(self.lang, "control_center.unsupported_7c5d5f").into(),
        args: String::new(),
        persist: PersistChange::None,
        prompt: None,
      }],
      MonitorSetting::Vrr | MonitorSetting::Hdr => vec![PickerOption {
        label: tr(self.lang, "control_center.unsupported_7c5d5f").into(),
        args: String::new(),
        persist: PersistChange::None,
        prompt: None,
      }],
      MonitorSetting::SdrBrightness | MonitorSetting::SdrSaturation => vec![PickerOption {
        label: tr(self.lang, "control_center.unsupported_7c5d5f").into(),
        args: String::new(),
        persist: PersistChange::None,
        prompt: None,
      }],
    }
  }

  /// Executes the `primary_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn primary_options(&self) -> Vec<PickerOption> {
    self
      .monitors
      .iter()
      .filter(|monitor| monitor.connected)
      .map(|monitor| {
        let is_primary = self.config.primary_monitor.as_deref() == Some(monitor.name.as_str());
        PickerOption {
          label: format!(
            "{} {}",
            monitor.name,
            if is_primary {
              tr(self.lang, "control_center.primary").to_string()
            } else {
              String::new()
            }
          ),
          args: monitor.name.clone(),
          persist: PersistChange::Primary,
          prompt: None,
        }
      })
      .collect()
  }

  /// Executes the `workspace_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn workspace_options(&self, index: usize) -> Vec<PickerOption> {
    let Some(monitor) = self.monitors.get(index) else {
      return vec![];
    };
    let name = monitor.name.clone();
    let owned = self.config.workspaces_of(&name);
    let mut options = Vec::new();
    for workspace in 1..=10 {
      let bound_here = owned.contains(&workspace);
      let bound_elsewhere = self
        .config
        .workspaces
        .iter()
        .filter(|(other, _)| *other != name)
        .any(|(_, ids)| ids.contains(&workspace));
      let label = if bound_here {
        format!(
          "{}  ·  {}",
          workspace,
          tr(self.lang, "control_center.this_monitor")
        )
      } else if bound_elsewhere {
        format!(
          "{}  ·  {}",
          workspace,
          tr(self.lang, "control_center.another_monitor")
        )
      } else {
        format!(
          "{}  ·  {}",
          workspace,
          tr(self.lang, "control_center.unbound")
        )
      };
      let persist = if bound_here {
        PersistChange::Workspace(workspace, None)
      } else {
        PersistChange::Workspace(workspace, Some(name.clone()))
      };
      options.push(PickerOption {
        label,
        args: String::new(),
        persist,
        prompt: None,
      });
    }
    options
  }

  /// Executes the `position_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn position_options(&self, index: usize) -> Vec<PickerOption> {
    let Some(monitor) = self.monitors.get(index) else {
      return vec![];
    };
    let mut options = vec![PickerOption {
      label: tr(self.lang, "control_center.top_left_corner_0_0").into(),
      args: monitor.position_args(0, 0),
      persist: PersistChange::Position(0, 0),
      prompt: None,
    }];
    for (other_index, other) in self.monitors.iter().enumerate() {
      if other_index == index || !other.connected {
        continue;
      }
      let placements = [
        (
          tr(self.lang, "control_center.to_the_right_of"),
          other.x + other.width as i32,
          other.y,
        ),
        (
          tr(self.lang, "control_center.to_the_left_of"),
          other.x - monitor.width as i32,
          other.y,
        ),
        (
          tr(self.lang, "control_center.below"),
          other.x,
          other.y + other.height as i32,
        ),
        (
          tr(self.lang, "control_center.above"),
          other.x,
          other.y - monitor.height as i32,
        ),
      ];
      for (label, x, y) in placements {
        options.push(PickerOption {
          label: format!("{label} {}", other.name),
          args: monitor.position_args(x, y),
          persist: PersistChange::Position(x, y),
          prompt: None,
        });
      }
    }
    options.sort_by(|left, right| left.label.cmp(&right.label));
    options
  }

  /// Applies the `apply_picker_selection` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_picker_selection(&mut self) {
    let Some((monitor_index, setting)) = self.monitor_ctx() else {
      return;
    };
    let options = self.picker_options();
    let Some(option) = options.get(self.selected).cloned() else {
      return;
    };
    if let Some(prompt) = option.prompt {
      self.open_prompt(prompt);
      return;
    }
    if setting == MonitorSetting::Primary {
      if let Some(monitor) = self.monitors.get(monitor_index) {
        let name = monitor.name.clone();
        self.config.primary_monitor = Some(name.clone());
        self.state.primary_monitor = Some(name.clone());
        let config = self.config.clone();
        let state = self.state.clone();
        let message = format!(
          "{}: {name}",
          tr(self.lang, "control_center.primary_monitor")
        );
        self.action = Some(self.manager.spawn(move |_| {
          backend::save_config(&config)?;
          backend::save_state(&state)?;
          Ok(JobData::Action(message))
        }));
      }
      self.page = DisplayPage::Detail(monitor_index);
      self.selected = 0;
      self.on_buttons = false;
      return;
    }
    let Some(monitor) = self.monitors.get(monitor_index).cloned() else {
      return;
    };
    let name = monitor.name.clone();
    let previous = monitor.clone();
    let prev_config = self.config.clone();
    let mut config = self.config.clone();
    Self::apply_persist(&mut config, &name, &option.persist);
    let risky = option_is_risky(&option.persist);
    if risky {
      self.revert = Some(RevertState {
        name: name.clone(),
        previous,
        previous_config: prev_config,
        deadline: Instant::now() + Duration::from_secs(REVERT_SECONDS),
      });
    }
    let _args = option.args.clone();
    let persist = option.persist.clone();
    let message = format!(
      "{} · {}",
      monitor.name,
      option.label.split("  ·  ").next().unwrap_or("")
    );
    self.config = config.clone();
    self.page = DisplayPage::Detail(monitor_index);
    self.selected = 0;
    self.on_buttons = false;
    self.action = Some(self.manager.spawn(move |_| {
      match persist {
        PersistChange::Dpms(true) => backend::set_dpms(&name, true)?,
        PersistChange::Dpms(_) => backend::set_dpms(&name, false)?,
        PersistChange::Workspace(workspace, target) => {
          backend::apply_workspace(workspace, target.as_deref())?;
          backend::save_config(&config)?;
        }
        _ => {
          backend::apply_all(&config)?;
        }
      }
      Ok(JobData::Action(message))
    }));
  }

  /// Applies the `apply_persist` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_persist(config: &mut PersistedConfig, name: &str, persist: &PersistChange) {
    match persist {
      PersistChange::Workspace(workspace, target) => {
        for (_, ids) in config.workspaces.iter_mut() {
          ids.retain(|id| id != workspace);
        }
        if let Some(target) = target {
          let mut ids = config.workspaces_of(target);
          if !ids.contains(workspace) {
            ids.push(*workspace);
          }
          ids.sort_unstable();
          config.set_workspaces(target, ids);
        }
      }
      _ => {
        let mut monitored = config.persisted(name);
        match persist {
          PersistChange::Mode(value) => monitored.mode = Some(value.clone()),
          PersistChange::Scale(value) => monitored.scale = Some(*value),
          PersistChange::Position(x, y) => monitored.position = Some(format!("{x}x{y}")),
          PersistChange::Vrr(value) => monitored.vrr = Some(*value),
          PersistChange::Hdr(value) => monitored.hdr = Some(*value),
          PersistChange::Mirror(value) => monitored.mirror = Some(value.clone()),
          PersistChange::BitDepth(value) => monitored.bitdepth = Some(*value),
          PersistChange::Disabled(value) => monitored.disabled = Some(*value),
          _ => return,
        }
        config.set_monitor(name, monitored);
      }
    }
  }

  /// Executes the `monitor_ctx` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn monitor_ctx(&self) -> Option<(usize, MonitorSetting)> {
    match self.page {
      DisplayPage::Picker { monitor, setting } => Some((monitor, setting)),
      DisplayPage::Detail(_)
      | DisplayPage::Home
      | DisplayPage::Profiles
      | DisplayPage::Prompt { .. } => None,
    }
  }

  /// Executes the `spawn_revert` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn spawn_revert(&mut self, config: PersistedConfig, message: String) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "control_center.reverting").into(),
    });
    self.action = Some(self.manager.spawn(move |_| {
      backend::apply_all(&config)?;
      Ok(JobData::Action(message))
    }));
  }

  /// Executes the `spawn_apply_with_config` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn spawn_apply_with_config(&mut self, config: PersistedConfig, message: String) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "control_center.applying_to_monitor").into(),
    });
    self.action = Some(self.manager.spawn(move |_| {
      backend::apply_all(&config)?;
      Ok(JobData::Action(message))
    }));
  }

  /// Executes the `vrr_supported` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn vrr_supported(&self) -> bool {
    self.version >= (0, 31)
  }

  /// Executes the `hdr_supported` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn hdr_supported(&self) -> bool {
    self.version >= (0, 42)
  }

  /// Executes the `home_entries` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn home_entries(&self) -> Vec<HomeEntry> {
    let mut entries: Vec<HomeEntry> = self
      .monitors
      .iter()
      .enumerate()
      .map(|(index, _)| HomeEntry::Live(index))
      .collect();
    for (name, persisted) in &self.config.monitors {
      if !self.monitors.iter().any(|monitor| &monitor.name == name) {
        entries.push(HomeEntry::Stale {
          name: name.clone(),
          persisted: persisted.clone(),
        });
      }
    }
    if !self.state.profiles.is_empty() || self.monitors.is_empty() {
      entries.push(HomeEntry::Profiles);
    }
    entries
  }

  /// Executes the `home_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn home_rows(&self) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for entry in self.home_entries() {
      match entry {
        HomeEntry::Live(index) => {
          let monitor = &self.monitors[index];
          let primary = if self.config.primary_monitor.as_deref() == Some(monitor.name.as_str()) {
            tr(self.lang, "control_center.primary_ead365")
          } else if monitor.focused {
            tr(self.lang, "control_center.focused")
          } else {
            ""
          };
          let dpms = if monitor.disabled || monitor.dpms_status == "off" {
            tr(self.lang, "control_center.off")
          } else {
            tr(self.lang, "control_center.on")
          };
          rows.push(format!(
            "{} {}  ·  {}x{} @ {:.3} Hz  ·  {}x{}  ·  escala {}  ·  {} {}",
            AppConfig::icon(argvus_tui::icons::MONITOR),
            monitor.name,
            monitor.width,
            monitor.height,
            monitor.refresh_rate,
            monitor.x,
            monitor.y,
            monitor.scale,
            dpms,
            primary,
          ));
        }
        HomeEntry::Stale { name, persisted } => {
          rows.push(format!(
            "{} {}  ·  {}  ·  {}",
            AppConfig::icon(argvus_tui::icons::ETHERNET),
            name,
            tr(self.lang, "control_center.disconnected"),
            persisted
              .mode
              .as_deref()
              .unwrap_or(tr(self.lang, "control_center.configured")),
          ));
        }
        HomeEntry::Profiles => {
          rows.push(format!(
            " {} {} ({})",
            AppConfig::icon(argvus_tui::icons::APPS),
            tr(self.lang, "control_center.profiles"),
            self.state.profiles.len()
          ));
        }
      }
    }
    if rows.is_empty() {
      rows.push(
        tr(
          self.lang,
          "control_center.no_monitors_found_is_hyprland_running",
        )
        .into(),
      );
    }
    rows
  }

  /// Executes the `profile_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn profile_rows(&self) -> Vec<String> {
    let mut rows = Vec::new();
    for profile in &self.state.profiles {
      let marker = if self.state.active_profile.as_deref() == Some(profile.name.as_str()) {
        "●"
      } else {
        "○"
      };
      rows.push(format!(" {marker} {}", profile.name));
    }
    rows.push(format!("+ {}", tr(self.lang, "control_center.new_profile")));
    rows
  }

  /// Executes the `selected_profile_index` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn selected_profile_index(&self) -> Option<usize> {
    let index = self.selected;
    if index < self.state.profiles.len() {
      Some(index)
    } else {
      None
    }
  }

  /// Applies the `apply_profile` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_profile(&mut self) {
    let Some(index) = self.selected_profile_index() else {
      self.status = Some(StatusMessage {
        kind: StatusKind::Warning,
        text: tr(self.lang, "control_center.select_a_profile_to_apply").into(),
      });
      return;
    };
    let profile = self.state.profiles[index].clone();
    let apply_wallpapers = profile.apply_wallpapers;
    self.config = profile.config.clone();
    self.state.primary_monitor = profile.config.primary_monitor.clone();
    self.state.active_profile = Some(profile.name.clone());
    let config = self.config.clone();
    let state = self.state.clone();
    let rules: Vec<String> = profile
      .config
      .monitors
      .iter()
      .map(|(name, persisted)| rule_from_persisted(name, persisted))
      .collect();
    let workspaces: Vec<(u32, String)> = profile
      .config
      .workspaces
      .iter()
      .flat_map(|(monitor, ids)| ids.iter().map(|id| (*id, monitor.clone())))
      .collect();
    let lang = self.lang;
    let profile_applied = tr(lang, "control_center.profile_applied").to_string();
    self.action = Some(self.manager.spawn(move |_| {
      for rule in &rules {
        backend::apply_keyword(rule)?;
      }
      for (workspace, monitor) in &workspaces {
        backend::apply_workspace(*workspace, Some(monitor))?;
      }
      if apply_wallpapers {
        apply_wallpapers_hook()?;
      }
      backend::save_config(&config)?;
      backend::save_state(&state)?;
      Ok(JobData::Action(format!(
        "{profile_applied} · {}",
        state.active_profile.as_deref().unwrap_or("")
      )))
    }));
    self.refresh();
  }

  /// Executes the `delete_profile` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn delete_profile(&mut self, index: usize) {
    if index < self.state.profiles.len() {
      self.state.profiles.remove(index);
    }
    let state = self.state.clone();
    self.action = Some(self.manager.spawn(move |_| {
      backend::save_state(&state)?;
      Ok(JobData::Action(String::new()))
    }));
  }

  /// Executes the `open_prompt` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn open_prompt(&mut self, goal: PromptGoal) {
    self.prompt_back = Some(self.page);
    self.prompt_buffer = self.prompt_prefill(goal);
    self.prompt_error = None;
    self.page = DisplayPage::Prompt { goal };
    self.selected = 0;
    self.on_buttons = false;
  }

  /// Executes the `prompt_prefill` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn prompt_prefill(&self, goal: PromptGoal) -> String {
    match goal {
      PromptGoal::Position(index) => self
        .monitors
        .get(index)
        .map(|monitor| {
          self
            .config
            .persisted(&monitor.name)
            .position
            .clone()
            .unwrap_or_else(|| format!("{},{}", monitor.x, monitor.y))
        })
        .unwrap_or_default(),
      PromptGoal::Scale(index) => self
        .monitors
        .get(index)
        .map(|monitor| {
          self
            .config
            .persisted(&monitor.name)
            .scale
            .map(|value| value.to_string())
            .unwrap_or_else(|| monitor.scale.to_string())
        })
        .unwrap_or_default(),
      PromptGoal::SdrBrightness(index) => self
        .monitors
        .get(index)
        .and_then(|monitor| {
          self
            .config
            .persisted(&monitor.name)
            .sdr_brightness
            .or(monitor.info.sdr_brightness)
            .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "1.0".into()),
      PromptGoal::SdrSaturation(index) => self
        .monitors
        .get(index)
        .and_then(|monitor| {
          self
            .config
            .persisted(&monitor.name)
            .sdr_saturation
            .or(monitor.info.sdr_saturation)
            .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "1.0".into()),
      PromptGoal::ProfileCreate => String::new(),
      PromptGoal::ProfileRename(index) => self
        .state
        .profiles
        .get(index)
        .map(|profile| profile.name.clone())
        .unwrap_or_default(),
    }
  }

  /// Executes the `reload_state` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn reload_state(&mut self) {
    self.state = backend::load_state();
    self.selected = self.selected.min(self.selection_len().saturating_sub(1));
  }

  /// Executes the `buttons` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn buttons(&self) -> Vec<(DisplayButton, Button)> {
    let secondary = ButtonKind::Secondary;
    let primary = ButtonKind::Primary;
    match self.page {
      DisplayPage::Home => vec![
        (
          DisplayButton::Refresh,
          Button::new(tr(self.lang, "control_center.refresh"), secondary),
        ),
        (
          DisplayButton::Profiles,
          Button::new(tr(self.lang, "control_center.profiles"), secondary),
        ),
      ],
      DisplayPage::Detail(index) => {
        let Some(monitor) = self.monitors.get(index) else {
          return vec![(
            DisplayButton::Remove,
            Button::new(tr(self.lang, "control_center.remove_config"), secondary),
          )];
        };
        if monitor.connected {
          vec![
            (
              DisplayButton::Apply,
              Button::new(tr(self.lang, "control_center.apply"), primary),
            ),
            (
              DisplayButton::Reset,
              Button::new(tr(self.lang, "control_center.default"), secondary),
            ),
          ]
        } else {
          vec![(
            DisplayButton::Remove,
            Button::new(tr(self.lang, "control_center.remove_config"), secondary),
          )]
        }
      }
      DisplayPage::Profiles => vec![
        (
          DisplayButton::ProfileNew,
          Button::new(tr(self.lang, "control_center.new"), primary),
        ),
        (
          DisplayButton::ProfileApply,
          Button::new(tr(self.lang, "control_center.apply"), secondary),
        ),
        (
          DisplayButton::ProfileRename,
          Button::new(tr(self.lang, "control_center.rename"), secondary),
        ),
        (
          DisplayButton::ProfileDelete,
          Button::new(tr(self.lang, "control_center.delete"), ButtonKind::Danger),
        ),
      ],
      DisplayPage::Picker { .. } | DisplayPage::Prompt { .. } => Vec::new(),
    }
  }

  /// Applies the `toggle_buttons` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `move_button` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn move_button(&mut self, delta: isize) {
    let count = self.buttons().len();
    if count == 0 {
      return;
    }
    self.button_selected =
      (self.button_selected as isize + delta).rem_euclid(count as isize) as usize;
  }

  /// Executes the `activate_button` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn activate_button(&mut self) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let actions = self.buttons();
    let Some((button, _)) = actions.get(self.button_selected) else {
      return;
    };
    match button {
      DisplayButton::Refresh => self.refresh(),
      DisplayButton::Apply => self.apply_button(),
      DisplayButton::Reset => self.reset_button(),
      DisplayButton::Remove => self.remove_button(),
      DisplayButton::Profiles => {
        self.page = DisplayPage::Profiles;
        self.selected = 0;
        self.on_buttons = false;
      }
      DisplayButton::ProfileNew => {
        if let DisplayPage::Profiles = self.page {
          self.open_prompt(PromptGoal::ProfileCreate);
        }
      }
      DisplayButton::ProfileApply => self.apply_profile(),
      DisplayButton::ProfileRename => {
        if let Some(index) = self.selected_profile_index() {
          self.open_prompt(PromptGoal::ProfileRename(index));
        } else if let DisplayPage::Profiles = self.page {
          self.open_prompt(PromptGoal::ProfileCreate);
        }
      }
      DisplayButton::ProfileDelete => {
        if let Some(index) = self.selected_profile_index() {
          self.confirm_profile = Some((index, ConfirmationState::default()));
        }
      }
    }
  }

  /// Executes the `remove_button` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn remove_button(&mut self) {
    let index = self.monitor_index();
    let name = match index {
      Some(index) if index < self.monitors.len() => Some(self.monitors[index].name.clone()),
      Some(index) => self.stale_name(index),
      None => None,
    };
    let Some(name) = name else {
      return;
    };
    self
      .config
      .monitors
      .retain(|(existing, _)| existing != &name);
    self
      .config
      .workspaces
      .retain(|(monitor, _)| monitor != &name);
    if self.config.primary_monitor.as_deref() == Some(&name) {
      self.config.primary_monitor = None;
      self.state.primary_monitor = None;
    }
    let config = self.config.clone();
    let state = self.state.clone();
    let lang = self.lang;
    let removed = tr(lang, "control_center.config_removed").to_string();
    self.action = Some(self.manager.spawn(move |_| {
      backend::save_config(&config)?;
      backend::save_state(&state)?;
      Ok(JobData::Action(format!("{removed} · {name}")))
    }));
    self.page = DisplayPage::Home;
    self.selected = 0;
    self.on_buttons = false;
  }

  /// Executes the `reset_button` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn reset_button(&mut self) {
    let Some(index) = self.monitor_index() else {
      return;
    };
    let Some(monitor) = self.monitors.get(index) else {
      return;
    };
    let name = monitor.name.clone();
    self
      .config
      .monitors
      .retain(|(existing, _)| existing != &name);
    let config = self.config.clone();
    self.spawn_apply_with_config(
      config,
      format!(
        "{} · {name}",
        tr(self.lang, "control_center.default_applied")
      ),
    );
  }

  /// Applies the `apply_button` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_button(&mut self) {
    let Some(index) = self.monitor_index() else {
      return;
    };
    let Some(monitor) = self.monitors.get(index) else {
      return;
    };
    let name = monitor.name.clone();
    let _persisted = self.config.persisted(&name);
    let previous = monitor.clone();
    let previous_config = self.config.clone();
    self.revert = Some(RevertState {
      name: name.clone(),
      previous,
      previous_config,
      deadline: Instant::now() + Duration::from_secs(REVERT_SECONDS),
    });
    self.spawn_apply_with_config(
      self.config.clone(),
      format!("{} · {name}", tr(self.lang, "control_center.applied")),
    );
  }

  /// Executes the `footer_hints` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn footer_hints(&self) -> String {
    match self.page {
      DisplayPage::Picker { .. } => tr(
        self.lang,
        "control_center.navigate_enter_apply_r_cancel_help",
      )
      .into(),
      DisplayPage::Prompt { .. } => tr(
        self.lang,
        "control_center.type_value_enter_confirm_esc_cancel",
      )
      .into(),
      DisplayPage::Profiles => tr(
        self.lang,
        "control_center.navigate_tab_actions_enter_new_r_reload_esc_back_help",
      )
      .into(),
      DisplayPage::Detail(_) => tr(
        self.lang,
        "control_center.navigate_tab_actions_enter_open_r_refresh_esc_back_help",
      )
      .into(),
      DisplayPage::Home => tr(
        self.lang,
        "control_center.navigate_enter_open_r_refresh_esc_back_help",
      )
      .into(),
    }
  }

  /// Executes the `breadcrumb` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "control_center.displays");
    match self.page {
      DisplayPage::Home => root.into(),
      DisplayPage::Detail(index) => {
        let name = self
          .monitors
          .get(index)
          .map(|monitor| monitor.name.clone())
          .or_else(|| self.stale_name(index))
          .unwrap_or_else(|| tr(self.lang, "control_center.monitor").into());
        format!("{root} > {name}")
      }
      DisplayPage::Picker { setting, .. } => {
        format!("{root} > {}", setting_label(self.lang, setting))
      }
      DisplayPage::Profiles => format!("{root} > {}", tr(self.lang, "control_center.profiles")),
      DisplayPage::Prompt { goal } => format!(
        "{root} > {}",
        match goal {
          PromptGoal::Position(_) => tr(self.lang, "control_center.position"),
          PromptGoal::Scale(_) => tr(self.lang, "control_center.scale"),
          PromptGoal::SdrBrightness(_) => tr(self.lang, "control_center.sdr_brightness"),
          PromptGoal::SdrSaturation(_) => tr(self.lang, "control_center.sdr_saturation"),
          PromptGoal::ProfileCreate => tr(self.lang, "control_center.new_profile"),
          PromptGoal::ProfileRename(_) => tr(self.lang, "control_center.rename_profile"),
        }
      ),
    }
  }

  /// Checks the condition represented by `is_picker` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn is_picker(&self) -> bool {
    matches!(self.page, DisplayPage::Picker { .. })
  }

  /// Renders `draw` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let body = shell(
      frame,
      area,
      &self.theme,
      &self.breadcrumb(),
      &self.footer_hints(),
    );
    if let DisplayPage::Prompt { goal } = self.page {
      self.draw_prompt(frame, body, goal);
      return;
    }
    if self.is_picker() {
      let options = self.picker_options();
      let options: Vec<String> = options.into_iter().map(|option| option.label).collect();
      let options = if options.is_empty() {
        vec![tr(self.lang, "control_center.no_options_available").into()]
      } else {
        options
      };
      list(
        frame,
        body,
        &self.theme,
        &options,
        self.selected.min(options.len().saturating_sub(1)),
      );
      self.overlays(frame, area);
      return;
    }
    match self.page {
      DisplayPage::Profiles => {
        let rows = self.profile_rows();
        let buttons = self.buttons();
        let raw_buttons: Vec<Button> = buttons.into_iter().map(|(_, button)| button).collect();
        let (list_area, button_area) = split_buttons(frame, body, &raw_buttons);
        let list_selection = if self.job.is_some() {
          usize::MAX
        } else {
          self.selected.min(rows.len().saturating_sub(1))
        };
        list(frame, list_area, &self.theme, &rows, list_selection);
        draw_buttons(frame, button_area, &raw_buttons, self, &self.theme);
        self.overlays(frame, area);
      }
      DisplayPage::Detail(_) => self.draw_detail(frame, area, body),
      DisplayPage::Home => {
        let rows = self.home_rows();
        let buttons = self.buttons();
        let raw_buttons: Vec<Button> = buttons.into_iter().map(|(_, button)| button).collect();
        let (list_area, button_area) = split_buttons(frame, body, &raw_buttons);
        let list_selection = if self.job.is_some() {
          usize::MAX
        } else {
          self.selected.min(rows.len().saturating_sub(1))
        };
        list(frame, list_area, &self.theme, &rows, list_selection);
        draw_buttons(frame, button_area, &raw_buttons, self, &self.theme);
        self.overlays(frame, area);
      }
      DisplayPage::Prompt { .. } | DisplayPage::Picker { .. } => {}
    }
  }

  /// Renders `draw_detail` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn draw_detail(&mut self, frame: &mut Frame, area: Rect, body: Rect) {
    let info = self.detail_info_rows();
    let buttons = self.buttons();
    let raw_buttons: Vec<Button> = buttons.into_iter().map(|(_, button)| button).collect();
    let (rest, button_area) = split_buttons(frame, body, &raw_buttons);
    let info_height = if info.is_empty() {
      0
    } else {
      (info.len() as u16).min(rest.height.saturating_sub(2))
    };
    let (info_area, list_area) = if info_height > 0 {
      let split =
        Layout::vertical([Constraint::Length(info_height), Constraint::Min(1)]).split(rest);
      (split[0], split[1])
    } else {
      (Rect::new(rest.x, rest.y, rest.width, 0), rest)
    };
    if info_height > 0 {
      let lines: Vec<Line> = std::iter::once(Line::from(""))
        .chain(info.iter().map(|(label, value)| {
          Line::from(vec![
            Span::styled(format!(" {label}"), Style::new().fg(self.theme.muted)),
            Span::raw("  ·  "),
            Span::styled(value.clone(), Style::new().fg(self.theme.foreground)),
          ])
        }))
        .collect();
      readonly(frame, info_area, &self.theme, &lines);
    }
    let rows = self.detail_rows();
    let list_selection = if self.job.is_some() {
      usize::MAX
    } else {
      self.selected.min(rows.len().saturating_sub(1))
    };
    list(frame, list_area, &self.theme, &rows, list_selection);
    draw_buttons(frame, button_area, &raw_buttons, self, &self.theme);
    self.overlays(frame, area);
  }

  /// Renders `draw_prompt` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn draw_prompt(&mut self, frame: &mut Frame, area: Rect, goal: PromptGoal) {
    let width = area.width.saturating_sub(6).clamp(34, 76);
    let height = 9u16.min(area.height);
    let x = area.x + (area.width - width).saturating_div(2);
    let y = area.y + (area.height.saturating_sub(height)).saturating_div(2);
    let box_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, box_area);
    let title = match goal {
      PromptGoal::Position(_) => tr(self.lang, "control_center.position_x_y"),
      PromptGoal::Scale(_) => tr(self.lang, "control_center.scale"),
      PromptGoal::SdrBrightness(_) => tr(self.lang, "control_center.sdr_brightness"),
      PromptGoal::SdrSaturation(_) => tr(self.lang, "control_center.sdr_saturation"),
      PromptGoal::ProfileCreate => tr(self.lang, "control_center.new_profile"),
      PromptGoal::ProfileRename(_) => tr(self.lang, "control_center.rename_profile"),
    };
    let display = if self.prompt_buffer.is_empty() {
      " ".into()
    } else {
      self.prompt_buffer.clone()
    };
    let mut lines = vec![Line::from("")];
    lines.push(Line::from(vec![
      Span::styled("> ", Style::new().fg(self.theme.accent)),
      Span::styled(
        display,
        Style::new()
          .fg(self.theme.foreground)
          .add_modifier(Modifier::BOLD),
      ),
    ]));
    lines.push(Line::from(""));
    if let Some(error) = &self.prompt_error {
      lines.push(Line::from(vec![Span::styled(
        argvus_tui::icons::icon_label(argvus_tui::icons::WARNING, error),
        Style::new().fg(self.theme.error),
      )]));
    }
    frame.render_widget(
      Paragraph::new(lines)
        .block(
          Block::bordered()
            .title(format!(" {title} "))
            .border_style(Style::new().fg(self.theme.border_active)),
        )
        .style(Style::new().bg(self.theme.background)),
      box_area,
    );
    self.overlays(frame, area);
  }

  /// Executes the `overlays` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn overlays(&self, frame: &mut Frame, area: Rect) {
    if let Some(revert) = &self.revert {
      draw_revert(frame, area, &self.theme, self.lang, revert);
      return;
    }
    if let Some((index, confirm)) = &self.confirm_profile {
      let name = self
        .state
        .profiles
        .get(*index)
        .map(|profile| profile.name.as_str())
        .unwrap_or("");
      let message = format!(
        "{}\n{}",
        tr(self.lang, "control_center.delete_this_profile"),
        name,
      );
      argvus_tui::components::draw_confirmation(
        frame,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(self.lang, "control_center.delete_profile"),
          message: &message,
          confirm_label: tr(self.lang, "control_center.delete"),
          cancel_label: tr(self.lang, "control_center.cancel"),
          confirm_selected: confirm.confirm_selected,
        },
      );
      return;
    }
    if let Some(s) = &self.status {
      status(frame, area, &self.theme, s);
    }
  }
}

/// Executes the `split_buttons` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn split_buttons(frame: &mut Frame, body: Rect, raw_buttons: &[Button]) -> (Rect, Option<Rect>) {
  let _ = frame;
  if raw_buttons.is_empty() {
    return (body, None);
  }
  let button_height = argvus_tui::buttons::height(raw_buttons, body.width).min(body.height);
  let split = Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(body);
  (split[0], Some(split[1]))
}

/// Renders `draw_buttons` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn draw_buttons(
  frame: &mut Frame,
  button_area: Option<Rect>,
  raw_buttons: &[Button],
  app: &DisplaysApp,
  theme: &Theme,
) {
  if let Some(button_area) = button_area {
    let focus = if app.on_buttons {
      app.button_selected
    } else {
      usize::MAX
    };
    argvus_tui::buttons::draw(frame, button_area, raw_buttons, focus, theme);
  }
}

/// Renders `draw_revert` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn draw_revert(frame: &mut Frame, area: Rect, theme: &Theme, lang: Lang, revert: &RevertState) {
  let remaining = revert
    .deadline
    .saturating_duration_since(Instant::now())
    .as_secs();
  let message = format!(
    "{} ({}s) · {} {}",
    tr(lang, "control_center.keep_this_configuration_enter_keep"),
    remaining,
    tr(
      lang,
      "control_center.reverting_automatically_if_no_key_is_pressed"
    ),
    revert.name,
  );
  let [top, _] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
  frame.render_widget(
    Paragraph::new(Line::from(message)).style(
      Style::new()
        .bg(theme.warning)
        .fg(theme.surface)
        .add_modifier(Modifier::BOLD),
    ),
    top,
  );
}

/// Executes the `resolution_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn resolution_options(monitor: &Monitor) -> Vec<PickerOption> {
  let mut unique: Vec<(u32, u32, f64)> = Vec::new();
  for mode in &monitor.modes {
    if !unique
      .iter()
      .any(|(w, h, _)| *w == mode.width && *h == mode.height)
    {
      unique.push((mode.width, mode.height, mode.refresh_rate));
    }
  }
  unique.sort_by(|left, right| {
    (right.0 * right.1).cmp(&(left.0 * left.1)).then(
      right
        .2
        .partial_cmp(&left.2)
        .unwrap_or(std::cmp::Ordering::Equal),
    )
  });
  unique
    .into_iter()
    .map(|(width, height, rate)| PickerOption {
      label: format!("{width}x{height}  ·  {:.3} Hz", rate),
      args: monitor.resolution_args(width, height, rate),
      persist: PersistChange::Mode(format!("{}x{}@{}", width, height, trimmed_rate(rate))),
      prompt: None,
    })
    .collect()
}

/// Executes the `refresh_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn refresh_options(monitor: &Monitor) -> Vec<PickerOption> {
  let mut modes = monitor.modes.clone();
  modes.sort_by(|left, right| {
    right
      .refresh_rate
      .partial_cmp(&left.refresh_rate)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  modes
    .into_iter()
    .map(|mode: crate::model::Mode| PickerOption {
      label: mode.label(),
      args: monitor.mode_id_args(&mode),
      persist: PersistChange::Mode(format!(
        "{}x{}@{}",
        mode.width,
        mode.height,
        trimmed_rate(mode.refresh_rate)
      )),
      prompt: None,
    })
    .collect()
}

/// Executes the `scale_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn scale_options(monitor: &Monitor) -> Vec<PickerOption> {
  [1.0_f64, 0.75, 1.1, 1.25, 1.5, 2.0]
    .into_iter()
    .map(|scale| PickerOption {
      label: format!("{scale}"),
      args: monitor.scale_args(scale),
      persist: PersistChange::Scale(scale),
      prompt: None,
    })
    .collect()
}

/// Executes the `enabled_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn enabled_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  vec![
    PickerOption {
      label: tr(lang, "control_center.enable").into(),
      args: monitor.enabled_with_current_args(),
      persist: PersistChange::Disabled(false),
      prompt: None,
    },
    PickerOption {
      label: tr(lang, "control_center.disable").into(),
      args: monitor.disabled_args(),
      persist: PersistChange::Disabled(true),
      prompt: None,
    },
  ]
}

/// Executes the `orientation_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn orientation_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  (0..8)
    .map(|transform| PickerOption {
      label: transform_label(lang, transform),
      args: monitor.transform_args(transform),
      persist: PersistChange::None,
      prompt: None,
    })
    .collect()
}

/// Executes the `mirror_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn mirror_options(monitor: &Monitor, monitors: &[Monitor], lang: Lang) -> Vec<PickerOption> {
  let mut options = vec![PickerOption {
    label: tr(lang, "control_center.no_mirror").into(),
    args: monitor.mirror_args(""),
    persist: PersistChange::Mirror(String::new()),
    prompt: None,
  }];
  for other in monitors {
    if other.name != monitor.name && other.connected {
      options.push(PickerOption {
        label: other.name.clone(),
        args: monitor.mirror_args(&other.name),
        persist: PersistChange::Mirror(other.name.clone()),
        prompt: None,
      });
    }
  }
  options
}

/// Executes the `bitdepth_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn bitdepth_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  let _ = lang;
  [8_i32, 10]
    .into_iter()
    .map(|bitdepth| PickerOption {
      label: format!("{bitdepth} bpp"),
      args: monitor.bitdepth_args(bitdepth),
      persist: PersistChange::BitDepth(bitdepth),
      prompt: None,
    })
    .collect()
}

/// Executes the `vrr_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn vrr_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  [0_i32, 1, 2]
    .into_iter()
    .map(|value| PickerOption {
      label: vrr_label(lang, value),
      args: monitor.vrr_args(value),
      persist: PersistChange::Vrr(value),
      prompt: None,
    })
    .collect()
}

/// Executes the `hdr_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn hdr_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  [0_i32, 1]
    .into_iter()
    .map(|value| PickerOption {
      label: hdr_label(lang, value),
      args: monitor.hdr_args(value),
      persist: PersistChange::Hdr(value),
      prompt: None,
    })
    .collect()
}

/// Executes the `dpms_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn dpms_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  let current_on = monitor.dpms_status != "off";
  vec![
    PickerOption {
      label: format!(
        "{}  {}",
        tr(lang, "control_center.on_5ddab3"),
        if current_on {
          tr(lang, "control_center.current_4cdf18").into()
        } else {
          String::new()
        }
      ),
      args: String::new(),
      persist: PersistChange::Dpms(true),
      prompt: None,
    },
    PickerOption {
      label: format!(
        "{}  {}",
        tr(lang, "control_center.off_688c4e"),
        if !current_on {
          tr(lang, "control_center.current_4cdf18").into()
        } else {
          String::new()
        }
      ),
      args: String::new(),
      persist: PersistChange::Dpms(false),
      prompt: None,
    },
  ]
}

/// Executes the `sdr_brightness_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn sdr_brightness_options(_lang: Lang, _monitor: &Monitor) -> Vec<PickerOption> {
  [0.25, 0.5, 0.75, 1.0]
    .into_iter()
    .map(|value| PickerOption {
      label: format!("{value:.2}"),
      args: String::new(),
      persist: PersistChange::None,
      prompt: None,
    })
    .collect()
}

/// Executes the `sdr_saturation_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn sdr_saturation_options(_lang: Lang, _monitor: &Monitor) -> Vec<PickerOption> {
  [0.5, 1.0, 1.5, 2.0]
    .into_iter()
    .map(|value| PickerOption {
      label: format!("{value:.2}"),
      args: String::new(),
      persist: PersistChange::None,
      prompt: None,
    })
    .collect()
}

/// Executes the `option_is_risky` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn option_is_risky(persist: &PersistChange) -> bool {
  matches!(
    persist,
    PersistChange::Mode(_)
      | PersistChange::Position(_, _)
      | PersistChange::Mirror(_)
      | PersistChange::Disabled(true)
      | PersistChange::BitDepth(_)
  )
}

/// Executes the `rule_from_persisted` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn rule_from_persisted(name: &str, persisted: &PersistedMonitor) -> String {
  let mode = persisted.mode.clone().unwrap_or_else(|| "auto".into());
  let position = persisted.position.clone().unwrap_or_else(|| "auto".into());
  let scale = persisted
    .scale
    .map(|value| value.to_string())
    .unwrap_or_else(|| "auto".into());
  let transform = persisted
    .transform
    .map(|value| value.to_string())
    .unwrap_or_else(|| "auto".into());
  let mut body = format!("{name}, {mode}, {position}, {scale}, transform, {transform}");
  if let Some(mirror) = &persisted.mirror {
    body.push_str(&format!(", mirror, {mirror}"));
  }
  if let Some(bitdepth) = persisted.bitdepth {
    body.push_str(&format!(", bitdepth, {bitdepth}"));
  }
  if let Some(vrr) = persisted.vrr {
    body.push_str(&format!(", vrr, {vrr}"));
  }
  if let Some(hdr) = persisted.hdr {
    body.push_str(&format!(", supports_hdr, {hdr}"));
  }
  if let Some(brightness) = persisted.sdr_brightness {
    body.push_str(&format!(", sdrbrightness, {brightness}"));
  }
  if let Some(saturation) = persisted.sdr_saturation {
    body.push_str(&format!(", sdrsaturation, {saturation}"));
  }
  if persisted.disabled == Some(true) {
    body = format!("{name}, disabled");
  }
  body
}

/// Applies the `apply_wallpapers_hook` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn apply_wallpapers_hook() -> Result<(), String> {
  let script = "/usr/bin/argvus-wallpapers-apply";
  if std::path::Path::new(script).exists() {
    let _ = std::process::Command::new(script).status();
  }
  Ok(())
}

/// Executes the `fingerprint` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn fingerprint(monitors: &[Monitor]) -> Vec<String> {
  monitors
    .iter()
    .map(|monitor| {
      format!(
        "{}:{}x{}:{}x{}:{}:{}",
        monitor.name,
        monitor.width,
        monitor.height,
        monitor.x,
        monitor.y,
        monitor.disabled,
        monitor.dpms_status
      )
    })
    .collect()
}

/// Executes the `trimmed_rate` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn trimmed_rate(rate: f64) -> String {
  let rounded = (rate * 1000.0).round() / 1000.0;
  if rounded.fract() < 0.0005 {
    format!("{}", rounded as u64)
  } else {
    format!("{rounded:.3}")
  }
}

/// Executes the `rate_of` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn rate_of(mode: &str) -> Option<f64> {
  mode.rsplit('@').next()?.parse::<f64>().ok()
}

/// Converts input data into `parse_position` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_position(buffer: &str) -> Option<(i32, i32)> {
  let (x, y) = buffer.split_once(',')?;
  Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Converts input data into `parse_number` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_number(buffer: &str) -> Option<f64> {
  buffer
    .trim()
    .parse::<f64>()
    .ok()
    .filter(|value| value.is_finite())
}

/// Executes the `prompt_allowed` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn prompt_allowed(goal: PromptGoal, character: char) -> bool {
  match goal {
    PromptGoal::Position(_) => {
      character.is_ascii_digit() || character == 'x' || character == ',' || character == '-'
    }
    PromptGoal::Scale(_) | PromptGoal::SdrBrightness(_) | PromptGoal::SdrSaturation(_) => {
      character.is_ascii_digit() || character == '.'
    }
    PromptGoal::ProfileCreate | PromptGoal::ProfileRename(_) => !character.is_control(),
  }
}

/// Executes the `setting_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn setting_label(lang: Lang, setting: MonitorSetting) -> String {
  match setting {
    MonitorSetting::Resolution => tr(lang, "control_center.resolution").into(),
    MonitorSetting::RefreshRate => tr(lang, "control_center.refresh_rate").into(),
    MonitorSetting::Scale => tr(lang, "control_center.scale").into(),
    MonitorSetting::Position => tr(lang, "control_center.position").into(),
    MonitorSetting::Primary => tr(lang, "control_center.primary_monitor").into(),
    MonitorSetting::Enabled => tr(lang, "control_center.monitor_enabled").into(),
    MonitorSetting::Mirror => tr(lang, "control_center.mirror").into(),
    MonitorSetting::BitDepth => tr(lang, "control_center.color_depth").into(),
    MonitorSetting::Vrr => tr(lang, "control_center.vrr").into(),
    MonitorSetting::Hdr => tr(lang, "control_center.hdr").into(),
    MonitorSetting::Dpms => "DPMS".into(),
    MonitorSetting::SdrBrightness => tr(lang, "control_center.sdr_brightness").into(),
    MonitorSetting::SdrSaturation => tr(lang, "control_center.sdr_saturation").into(),
    MonitorSetting::Workspaces => tr(lang, "control_center.workspaces").into(),
    MonitorSetting::Orientation => tr(lang, "control_center.orientation").into(),
  }
}

/// Executes the `transform_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn transform_label(lang: Lang, transform: i32) -> String {
  match transform {
    0 => tr(lang, "control_center.normal").into(),
    1 => tr(lang, "control_center.90").into(),
    2 => tr(lang, "control_center.180").into(),
    3 => tr(lang, "control_center.270").into(),
    4 => tr(lang, "control_center.flipped").into(),
    5 => tr(lang, "control_center.flipped_90").into(),
    6 => tr(lang, "control_center.flipped_180").into(),
    7 => tr(lang, "control_center.flipped_270").into(),
    _ => format!("{transform}"),
  }
}

/// Executes the `vrr_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn vrr_label(lang: Lang, vrr: i32) -> String {
  match vrr {
    0 => tr(lang, "control_center.disabled").into(),
    1 => tr(lang, "control_center.enabled").into(),
    2 => tr(lang, "control_center.fullscreen_only").into(),
    _ => format!("{vrr}"),
  }
}

/// Executes the `hdr_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn hdr_label(lang: Lang, hdr: i32) -> String {
  if hdr != 0 {
    tr(lang, "control_center.forced").into()
  } else {
    tr(lang, "control_center.auto_d2c2e5").into()
  }
}

/// Processes `on_off` in this module's event flow. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn on_off(lang: Lang, on: bool) -> String {
  if on {
    tr(lang, "control_center.yes").into()
  } else {
    tr(lang, "control_center.no").into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::Mode;

  /// Executes the `monitor` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn monitor(name: &str) -> Monitor {
    Monitor {
      id: 0,
      name: name.into(),
      x: 0,
      y: 0,
      width: 1920,
      height: 1080,
      refresh_rate: 60.0,
      scale: 1.0,
      transform: 0,
      focused: false,
      vrr: 0,
      dpms_status: "on".into(),
      disabled: false,
      modes: vec![
        Mode {
          id: 1,
          width: 1920,
          height: 1080,
          refresh_rate: 60.0,
          bit_depth: 8,
        },
        Mode {
          id: 2,
          width: 1280,
          height: 720,
          refresh_rate: 59.94,
          bit_depth: 10,
        },
      ],
      connected: true,
      info: crate::model::MonitorInfo::default(),
      active_workspace: Some(1),
    }
  }

  /// Executes the `app_with` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn app_with(job: bool) -> DisplaysApp {
    let mut app = DisplaysApp::new(Lang::for_locale("en-US"), Theme::load());
    if !job {
      app.job = None;
      app.action = None;
      app.hotplug = None;
    }
    app.monitors = vec![monitor("eDP-1")];
    app
  }

  #[test]
  /// Executes the `versions_gate_fancy_features` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn versions_gate_fancy_features() {
    let mut app = app_with(true);
    assert!(!app.vrr_supported() && !app.hdr_supported());
    app.version = (0, 42);
    assert!(app.vrr_supported() && app.hdr_supported());
  }

  #[test]
  /// Executes the `home_rows_merge_profiles_entry` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn home_rows_merge_profiles_entry() {
    let mut app = app_with(false);
    app.state.profiles.push(MonitorProfile {
      name: "Trabalho".into(),
      apply_wallpapers: false,
      config: PersistedConfig::default(),
    });
    let rows = app.home_rows();
    assert!(rows[0].contains("eDP-1"));
    assert!(!rows[0].contains("Trabalho"));
    assert!(rows.last().unwrap().contains("Profiles"));
  }

  #[test]
  /// Executes the `risky_persist_changes_arm_revert` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn risky_persist_changes_arm_revert() {
    assert!(option_is_risky(&PersistChange::Mode("x".into())));
    assert!(option_is_risky(&PersistChange::Position(1, 2)));
    assert!(option_is_risky(&PersistChange::Disabled(true)));
    assert!(!option_is_risky(&PersistChange::Scale(1.0)));
    assert!(!option_is_risky(&PersistChange::Vrr(1)));
    assert!(!option_is_risky(&PersistChange::Hdr(1)));
    assert!(!option_is_risky(&PersistChange::Workspace(3, None)));
  }

  #[test]
  /// Executes the `prompt_validation_rejects_bad_positions` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn prompt_validation_rejects_bad_positions() {
    assert_eq!(parse_position("1920,0"), Some((1920, 0)));
    assert_eq!(parse_position("0x0"), None);
    assert_eq!(parse_position("abc"), None);
    assert_eq!(parse_number("1.5"), Some(1.5));
    assert_eq!(parse_number("x"), None);
  }

  #[test]
  /// Executes the `workspace_binding_persist_moves_workspace` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn workspace_binding_persist_moves_workspace() {
    let mut config = PersistedConfig::default();
    config.set_workspaces("eDP-1", vec![1, 2]);
    config.set_workspaces("DP-1", vec![3]);
    DisplaysApp::apply_persist(
      &mut config,
      "DP-1",
      &PersistChange::Workspace(2, Some("DP-1".into())),
    );
    assert_eq!(config.workspaces_of("eDP-1"), vec![1]);
    assert_eq!(config.workspaces_of("DP-1"), vec![2, 3]);
    DisplaysApp::apply_persist(&mut config, "DP-1", &PersistChange::Workspace(2, None));
    assert_eq!(config.workspaces_of("DP-1"), vec![3]);
  }

  #[test]
  /// Executes the `profile_rule_replays_persisted_fields` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn profile_rule_replays_persisted_fields() {
    let persisted = PersistedMonitor {
      mode: Some("1920x1080@144".into()),
      position: Some("0x0".into()),
      scale: Some(1.25),
      transform: Some(1),
      vrr: Some(1),
      hdr: Some(1),
      bitdepth: Some(10),
      disabled: Some(false),
      ..Default::default()
    };
    let rule = rule_from_persisted("eDP-1", &persisted);
    assert!(
      rule.starts_with("eDP-1, 1920x1080@144, 0x0, 1.25, transform, 1"),
      "{rule}"
    );
    assert!(rule.contains("vrr, 1"), "{rule}");
    assert!(rule.contains("bitdepth, 10"), "{rule}");
    assert!(rule.contains("supports_hdr, 1"), "{rule}");
    let disabled = PersistedMonitor {
      disabled: Some(true),
      ..Default::default()
    };
    assert_eq!(rule_from_persisted("eDP-1", &disabled), "eDP-1, disabled");
  }

  #[test]
  /// Executes the `detail_settings_include_supported_and_gated` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn detail_settings_include_supported_and_gated() {
    let mut app = app_with(false);
    app.version = (0, 42);
    app.page = DisplayPage::Detail(0);
    let settings = app.detail_settings();
    assert!(settings.contains(&MonitorSetting::Workspaces));
    assert!(settings.contains(&MonitorSetting::SdrBrightness));
    assert!(settings.contains(&MonitorSetting::SdrSaturation));
    assert!(settings.contains(&MonitorSetting::Enabled));
    assert!(settings.contains(&MonitorSetting::Hdr));
    app.version = (0, 30);
    let settings = app.detail_settings();
    assert!(!settings.contains(&MonitorSetting::Vrr));
    assert!(!settings.contains(&MonitorSetting::Hdr));
    assert!(!settings.contains(&MonitorSetting::SdrBrightness));
  }

  #[test]
  /// Executes the `primary_option_rows_list_every_connected_monitor` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn primary_option_rows_list_every_connected_monitor() {
    let mut app = app_with(false);
    app.monitors.push(Monitor {
      name: "DP-1".into(),
      x: 1920,
      y: 0,
      ..monitor("DP-1")
    });
    let options = app.primary_options();
    assert_eq!(options.len(), 2);
    assert!(
      options
        .iter()
        .all(|option| option.persist == PersistChange::Primary)
    );
  }

  #[test]
  /// Executes the `workspace_editor_marks_this_monitor_and_others` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn workspace_editor_marks_this_monitor_and_others() {
    let mut app = app_with(false);
    app.monitors.push(Monitor {
      name: "DP-1".into(),
      x: 1920,
      y: 0,
      ..monitor("DP-1")
    });
    app.config.set_workspaces("eDP-1", vec![1, 2]);
    app.config.set_workspaces("DP-1", vec![3]);
    let options = app.workspace_options(0);
    assert!(
      options[0].label.contains("this monitor"),
      "{:?}",
      options[0].label
    );
    assert!(
      options[2].label.contains("another monitor"),
      "{:?}",
      options[2].label
    );
    assert!(
      options[4].label.contains("unbound"),
      "{:?}",
      options[4].label
    );
  }

  #[test]
  /// Executes the `app_navigates_home_to_detail_picker_and_back` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn app_navigates_home_to_detail_picker_and_back() {
    let mut app = app_with(false);
    app.handle(KeyCode::Enter);
    assert!(matches!(app.page, DisplayPage::Detail(0)));
    app.handle(KeyCode::Enter);
    assert!(matches!(
      app.page,
      DisplayPage::Picker {
        setting: MonitorSetting::Resolution,
        ..
      }
    ));
    app.handle(KeyCode::Esc);
    assert!(matches!(app.page, DisplayPage::Detail(0)));
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, DisplayPage::Home);
  }
}
