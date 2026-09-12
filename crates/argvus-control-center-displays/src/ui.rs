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

const REVERT_SECONDS: u64 = 15;
const HOTPLUG_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, PartialEq)]
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
struct PickerOption {
  label: String,
  args: String,
  persist: PersistChange,
  /// When set, selecting this row opens the free-form editor goal instead.
  prompt: Option<PromptGoal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

struct RevertState {
  name: String,
  previous: Monitor,
  previous_config: PersistedConfig,
  deadline: Instant,
}

enum HomeEntry {
  Live(usize),
  Stale {
    name: String,
    persisted: PersistedMonitor,
  },
  Profiles,
}

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
  pub fn reload(&mut self) {
    self.refresh();
  }

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
      last_hotplug: Instant::now().checked_sub(HOTPLUG_INTERVAL * 6).unwrap_or(Instant::now()),
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
      text: tr(lang, "Carregando monitores...", "Loading monitors...").into(),
    });
  }

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
        Err(e) => self.status = Some(StatusMessage {
          kind: StatusKind::Error,
          text: e,
        }),
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
          tr(self.lang, "Revertido", "Reverted"),
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
            text: tr(
              self.lang,
              "Monitores alterados (hotplug)",
              "Monitors changed (hotplug)",
            )
            .into(),
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

  pub fn handle(&mut self, key: KeyCode) -> bool {
    if let Some((index, mut confirm)) = self.confirm_profile.take() {
      match confirm.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.delete_profile(index);
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: tr(self.lang, "Perfil excluído", "Profile deleted").into(),
          });
          self.selected = self.selected.min(self.state.profiles.len().saturating_sub(1));
        }
        ConfirmationOutcome::Cancelled => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Info,
            text: tr(self.lang, "Exclusão cancelada", "Deletion cancelled").into(),
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
                tr(self.lang, "Configuração mantida", "Configuration kept")
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
                tr(self.lang, "Revertido", "Reverted"),
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
        KeyCode::Enter
          if self.selected == self.state.profiles.len() => self.open_prompt(PromptGoal::ProfileCreate),
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

  fn commit_prompt(&mut self, goal: PromptGoal) {
    let buffer = self.prompt_buffer.clone();
    match goal {
      PromptGoal::Position(index) => {
        let Some((x, y)) = parse_position(&buffer) else {
          self.prompt_error = Some(
            tr(
              self.lang,
              "Posição inválida — use o formato X,Y (ex.: 1920,0)",
              "Invalid position — use X,Y (e.g. 1920,0)",
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
          tr(self.lang, "Posição", "Position")
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
              "Valor inválido — use um número (ex.: 1.25)",
              "Invalid value — use a number (e.g. 1.25)",
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
          tr(self.lang, "Escala", "Scale")
        );
        self.page = DisplayPage::Detail(index);
        self.selected = 0;
        self.on_buttons = false;
        self.spawn_apply_with_config(config, message);
      }
      PromptGoal::SdrBrightness(index) => {
        let Some(value) = parse_number(&buffer) else {
          self.prompt_error = Some(
            tr(
              self.lang,
              "Valor inválido — use 0.0 a 1.0",
              "Invalid value — use 0.0 to 1.0",
            )
            .into(),
          );
          return;
        };
        self.apply_sdr(index, |persisted| persisted.sdr_brightness = Some(value.clamp(0.0, 1.0)));
      }
      PromptGoal::SdrSaturation(index) => {
        let Some(value) = parse_number(&buffer) else {
          self.prompt_error = Some(
            tr(
              self.lang,
              "Valor inválido — use 0.0 a 2.0",
              "Invalid value — use 0.0 to 2.0",
            )
            .into(),
          );
          return;
        };
        self.apply_sdr(index, |saturation| saturation.sdr_saturation = Some(value.clamp(0.0, 2.0)));
      }
      PromptGoal::ProfileCreate => {
        if buffer.trim().is_empty() {
          self.prompt_error = Some(
            tr(self.lang, "O nome do perfil não pode ser vazio", "Profile name cannot be empty")
              .into(),
          );
          return;
        }
        if self.state.profile_index(buffer.trim()) .is_some() {
          self.prompt_error = Some(
            tr(self.lang, "Um perfil com esse nome já existe", "A profile with this name already exists")
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
        self.spawn_save_state(state, format!("{} · {}", tr(self.lang, "Perfil criado", "Profile created"), buffer.trim()));
        self.page = DisplayPage::Profiles;
        self.selected = self.state.profiles.len().saturating_sub(1);
        self.prompt_buffer.clear();
        self.prompt_error = None;
        self.prompt_back = None;
        self.on_buttons = false;
      }
      PromptGoal::ProfileRename(index) => {
        if buffer.trim().is_empty() {
          self.prompt_error = Some(
            tr(self.lang, "O nome do perfil não pode ser vazio", "Profile name cannot be empty")
              .into(),
          );
          return;
        }
        if let Some(profile) = self.state.profiles.get_mut(index) {
          profile.name = buffer.trim().to_string();
        }
        let state = self.state.clone();
        self.spawn_save_state(state, format!("{} · {}", tr(self.lang, "Perfil renomeado", "Profile renamed"), buffer.trim()));
        self.page = DisplayPage::Profiles;
        self.selected = index;
        self.prompt_buffer.clear();
        self.prompt_error = None;
        self.prompt_back = None;
        self.on_buttons = false;
      }
    }
  }

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
    let message = format!("{} · {name}", tr(self.lang, "SDR", "SDR"));
    self.page = DisplayPage::Detail(index);
    self.selected = 0;
    self.on_buttons = false;
    self.spawn_apply_with_config(config, message);
  }

  fn spawn_save_state(&mut self, state: crate::model::DisplayState, message: String) {
    self.action = Some(self.manager.spawn(move |_| {
      backend::save_state(&state)?;
      Ok(JobData::Action(message))
    }));
  }

  fn exit_picker(&mut self) {
    let page = self.page;
    if let DisplayPage::Picker { monitor, .. } = page {
      self.page = DisplayPage::Detail(monitor);
    }
    self.selected = 0;
    self.on_buttons = false;
  }

  fn selection_len(&self) -> usize {
    match self.page {
      DisplayPage::Home => self.home_entries().len(),
      DisplayPage::Detail(_) => self.detail_settings().len(),
      DisplayPage::Profiles => self.profile_rows().len(),
      DisplayPage::Picker { .. } => self.picker_options().len(),
      DisplayPage::Prompt { .. } => 0,
    }
  }

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

  fn stale_start(&self) -> usize {
    self.monitors.len()
  }

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

  fn detail_setting(&self, row: usize) -> Option<MonitorSetting> {
    let settings = self.detail_settings();
    settings.get(row).copied()
  }

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

  fn monitor_index(&self) -> Option<usize> {
    match self.page {
      DisplayPage::Detail(index) | DisplayPage::Picker { monitor: index, .. } => Some(index),
      DisplayPage::Home | DisplayPage::Profiles | DisplayPage::Prompt { .. } => None,
    }
  }

  fn detail_info_rows(&self) -> Vec<(String, String)> {
    let Some(index) = self.monitor_index() else {
      return vec![];
    };
    let Some(monitor) = self.monitors.get(index) else {
      return vec![];
    };
    monitor.info_rows()
  }

  fn detail_rows(&self) -> Vec<String> {
    let Some(index) = self.monitor_index() else {
      return vec![];
    };
    let Some(monitor) = self.monitors.get(index) else {
      let name = self.stale_name(index).unwrap_or_default();
      return vec![format!(
        " {} {} {name}",
        AppConfig::icon("🔌"),
        tr(self.lang, "Monitor desconectado", "Monitor disconnected")
      )];
    };
    if !monitor.connected {
      return vec![format!(
        " {} {}",
        AppConfig::icon("🔌"),
        tr(self.lang, "Monitor desconectado", "Monitor disconnected")
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
      .unwrap_or_else(|| tr(self.lang, "nenhum", "none").into());
    let mut rows = vec![
      format!(
        " {}  {}  ·  {mode}",
        tr(self.lang, "Resolução", "Resolution"),
        monitor.name
      ),
      format!(
        " {}  ·  {:.3} Hz",
        tr(self.lang, "Taxa de atualização", "Refresh rate"),
        rate
      ),
      format!(" {}  ·  {}", tr(self.lang, "Escala", "Scale"), scale),
      format!(" {}  ·  {}", tr(self.lang, "Posição", "Position"), position),
      format!(
        " {}  ·  {}",
        tr(self.lang, "Orientação", "Orientation"),
        transform_label(self.lang, transform)
      ),
      format!(
        " {}  ·  {}",
        tr(self.lang, "Monitor ligado", "Monitor enabled"),
        on_off(self.lang, persisted.disabled != Some(true))
      ),
      format!(
        " {}  ·  {mirror}",
        tr(self.lang, "Espelhamento", "Mirror")
      ),
      format!(
        " {}  ·  {}",
        tr(self.lang, "Profundidade de cor", "Color depth"),
        self.bitdepth_label(monitor, &persisted)
      ),
    ];
    if self.vrr_supported() {
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "VRR", "VRR"),
        vrr_label(self.lang, vrr)
      ));
    }
    if self.hdr_supported() && monitor.has_10bit() {
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "HDR", "HDR"),
        hdr_label(self.lang, hdr)
      ));
    }
    rows.push(format!(
      " {}  ·  {}",
      tr(self.lang, "DPMS", "DPMS"),
      if monitor.dpms_status == "off" {
        tr(self.lang, "desligado", "off")
      } else {
        tr(self.lang, "ligado", "on")
      }
    ));
    if self.hdr_supported() && monitor.has_10bit() {
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "Brilho SDR", "SDR brightness"),
        persisted
          .sdr_brightness
          .or(monitor.info.sdr_brightness)
          .map(|value| format!("{value:.2}"))
          .unwrap_or_else(|| tr(self.lang, "automático", "auto").into())
      ));
      rows.push(format!(
        " {}  ·  {}",
        tr(self.lang, "Saturação SDR", "SDR saturation"),
        persisted
          .sdr_saturation
          .or(monitor.info.sdr_saturation)
          .map(|value| format!("{value:.2}"))
          .unwrap_or_else(|| tr(self.lang, "automático", "auto").into())
      ));
    }
    let workspaces = self.config.workspaces_of(&monitor.name);
    rows.push(format!(
      " {}  ·  {}",
      tr(self.lang, "Workspaces", "Workspaces"),
      if workspaces.is_empty() {
        tr(self.lang, "sem vínculo", "unbound").into()
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
      tr(self.lang, "Monitor primário", "Primary monitor"),
      on_off(self.lang, primary)
    ));
    rows
  }

  fn bitdepth_label(&self, monitor: &Monitor, persisted: &PersistedMonitor) -> String {
    if !monitor.has_10bit() {
      return tr(self.lang, "não suportado", "unsupported").into();
    }
    match persisted.bitdepth {
      Some(10) => "10".into(),
      Some(8) => "8".into(),
      _ if monitor.info.current_format.contains("10") => "10".into(),
      _ => "8".into(),
    }
  }

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
          label: format!(
            "✎ {}",
            tr(self.lang, "Personalizar...", "Custom...")
          ),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::Scale(monitor_index)),
        });
        options
      }
      MonitorSetting::Position => {
        let mut options = self.position_options(monitor_index);
        options.push(PickerOption {
          label: format!(
            "✎ {}",
            tr(self.lang, "Personalizar...", "Custom...")
          ),
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
          label: format!(
            "✎ {}",
            tr(self.lang, "Personalizar...", "Custom...")
          ),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::SdrBrightness(monitor_index)),
        });
        options
      }
      MonitorSetting::SdrSaturation if self.hdr_supported() && monitor.has_10bit() => {
        let mut options = sdr_saturation_options(self.lang, monitor);
        options.push(PickerOption {
          label: format!(
            "✎ {}",
            tr(self.lang, "Personalizar...", "Custom...")
          ),
          args: String::new(),
          persist: PersistChange::None,
          prompt: Some(PromptGoal::SdrSaturation(monitor_index)),
        });
        options
      }
      MonitorSetting::Workspaces => self.workspace_options(monitor_index),
      MonitorSetting::BitDepth => vec![PickerOption {
        label: tr(self.lang, "Não suportado", "Unsupported").into(),
        args: String::new(),
        persist: PersistChange::None,
        prompt: None,
      }],
      MonitorSetting::Vrr | MonitorSetting::Hdr => vec![PickerOption {
        label: tr(self.lang, "Não suportado", "Unsupported").into(),
        args: String::new(),
        persist: PersistChange::None,
        prompt: None,
      }],
      MonitorSetting::SdrBrightness | MonitorSetting::SdrSaturation => vec![PickerOption {
        label: tr(self.lang, "Não suportado", "Unsupported").into(),
        args: String::new(),
        persist: PersistChange::None,
        prompt: None,
      }],
    }
  }

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
              tr(self.lang, "· primário", "· primary").to_string()
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
          tr(self.lang, "este monitor", "this monitor")
        )
      } else if bound_elsewhere {
        format!("{}  ·  {}", workspace, tr(self.lang, "outro monitor", "another monitor"))
      } else {
        format!("{}  ·  {}", workspace, tr(self.lang, "sem vínculo", "unbound"))
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

  fn position_options(&self, index: usize) -> Vec<PickerOption> {
    let Some(monitor) = self.monitors.get(index) else {
      return vec![];
    };
    let mut options = vec![PickerOption {
      label: tr(
        self.lang,
        "Canto superior esquerdo (0, 0)",
        "Top-left corner (0, 0)",
      )
      .into(),
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
          tr(self.lang, "À direita de", "To the right of"),
          other.x + other.width as i32,
          other.y,
        ),
        (
          tr(self.lang, "À esquerda de", "To the left of"),
          other.x - monitor.width as i32,
          other.y,
        ),
        (
          tr(self.lang, "Abaixo de", "Below"),
          other.x,
          other.y + other.height as i32,
        ),
        (
          tr(self.lang, "Acima de", "Above"),
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
        let message = format!("{}: {name}", tr(self.lang, "Monitor primário", "Primary monitor"));
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

  fn monitor_ctx(&self) -> Option<(usize, MonitorSetting)> {
    match self.page {
      DisplayPage::Picker { monitor, setting } => Some((monitor, setting)),
      DisplayPage::Detail(_) | DisplayPage::Home | DisplayPage::Profiles | DisplayPage::Prompt { .. } => None,
    }
  }

  fn spawn_revert(&mut self, config: PersistedConfig, message: String) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Revertendo...", "Reverting...").into(),
    });
    self.action = Some(self.manager.spawn(move |_| {
      backend::apply_all(&config)?;
      Ok(JobData::Action(message))
    }));
  }

  fn spawn_apply_with_config(&mut self, config: PersistedConfig, message: String) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Aplicando ao monitor...",
        "Applying to monitor...",
      )
      .into(),
    });
    self.action = Some(self.manager.spawn(move |_| {
      backend::apply_all(&config)?;
      Ok(JobData::Action(message))
    }));
  }

  fn vrr_supported(&self) -> bool {
    self.version >= (0, 31)
  }

  fn hdr_supported(&self) -> bool {
    self.version >= (0, 42)
  }

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

  fn home_rows(&self) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for entry in self.home_entries() {
      match entry {
        HomeEntry::Live(index) => {
          let monitor = &self.monitors[index];
          let primary = if self.config.primary_monitor.as_deref() == Some(monitor.name.as_str()) {
            tr(self.lang, "primário", "primary")
          } else if monitor.focused {
            tr(self.lang, "focado", "focused")
          } else {
            ""
          };
          let dpms = if monitor.disabled || monitor.dpms_status == "off" {
            tr(self.lang, "desligado", "off")
          } else {
            tr(self.lang, "ligado", "on")
          };
          rows.push(format!(
            "{} {}  ·  {}x{} @ {:.3} Hz  ·  {}x{}  ·  escala {}  ·  {} {}",
            AppConfig::icon("🖥️"),
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
            AppConfig::icon("🔌"),
            name,
            tr(self.lang, "desconectado", "disconnected"),
            persisted
              .mode
              .as_deref()
              .unwrap_or(tr(self.lang, "configurado", "configured")),
          ));
        }
        HomeEntry::Profiles => {
          rows.push(format!(
            " {} {} ({})",
            AppConfig::icon("🗂️"),
            tr(self.lang, "Perfis", "Profiles"),
            self.state.profiles.len()
          ));
        }
      }
    }
    if rows.is_empty() {
      rows.push(
        tr(
          self.lang,
          "Nenhum monitor encontrado — o Hyprland está em execução?",
          "No monitors found — is Hyprland running?",
        )
        .into(),
      );
    }
    rows
  }

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
    rows.push(format!(
      "+ {}",
      tr(self.lang, "Novo perfil", "New profile")
    ));
    rows
  }

  fn selected_profile_index(&self) -> Option<usize> {
    let index = self.selected;
    if index < self.state.profiles.len() {
      Some(index)
    } else {
      None
    }
  }

  fn apply_profile(&mut self) {
    let Some(index) = self.selected_profile_index() else {
      self.status = Some(StatusMessage {
        kind: StatusKind::Warning,
        text: tr(
          self.lang,
          "Selecione um perfil para aplicar",
          "Select a profile to apply",
        )
        .into(),
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
    let profile_applied = tr(lang, "Perfil aplicado", "Profile applied").to_string();
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

  fn open_prompt(&mut self, goal: PromptGoal) {
    self.prompt_back = Some(self.page);
    self.prompt_buffer = self.prompt_prefill(goal);
    self.prompt_error = None;
    self.page = DisplayPage::Prompt { goal };
    self.selected = 0;
    self.on_buttons = false;
  }

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

  fn reload_state(&mut self) {
    self.state = backend::load_state();
    self.selected = self.selected.min(self.selection_len().saturating_sub(1));
  }

  fn buttons(&self) -> Vec<(DisplayButton, Button)> {
    let secondary = ButtonKind::Secondary;
    let primary = ButtonKind::Primary;
    match self.page {
      DisplayPage::Home => vec![
        (
          DisplayButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), secondary),
        ),
        (
          DisplayButton::Profiles,
          Button::new(tr(self.lang, "Perfis", "Profiles"), secondary),
        ),
      ],
      DisplayPage::Detail(index) => {
        let Some(monitor) = self.monitors.get(index) else {
          return vec![(
            DisplayButton::Remove,
            Button::new(
              tr(self.lang, "Remover configuração", "Remove config"),
              secondary,
            ),
          )];
        };
        if monitor.connected {
          vec![
            (
              DisplayButton::Apply,
              Button::new(tr(self.lang, "Aplicar", "Apply"), primary),
            ),
            (
              DisplayButton::Reset,
              Button::new(tr(self.lang, "Padrão", "Default"), secondary),
            ),
          ]
        } else {
          vec![(
            DisplayButton::Remove,
            Button::new(
              tr(self.lang, "Remover configuração", "Remove config"),
              secondary,
            ),
          )]
        }
      }
      DisplayPage::Profiles => vec![
        (
          DisplayButton::ProfileNew,
          Button::new(tr(self.lang, "Novo", "New"), primary),
        ),
        (
          DisplayButton::ProfileApply,
          Button::new(tr(self.lang, "Aplicar", "Apply"), secondary),
        ),
        (
          DisplayButton::ProfileRename,
          Button::new(tr(self.lang, "Renomear", "Rename"), secondary),
        ),
        (
          DisplayButton::ProfileDelete,
          Button::new(tr(self.lang, "Excluir", "Delete"), ButtonKind::Danger),
        ),
      ],
      DisplayPage::Picker { .. } | DisplayPage::Prompt { .. } => Vec::new(),
    }
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
    let removed = tr(lang, "Configuração removida", "Config removed").to_string();
    self.action = Some(self.manager.spawn(move |_| {
      backend::save_config(&config)?;
      backend::save_state(&state)?;
      Ok(JobData::Action(format!("{removed} · {name}")))
    }));
    self.page = DisplayPage::Home;
    self.selected = 0;
    self.on_buttons = false;
  }

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
      format!("{} · {name}", tr(self.lang, "Padrão aplicado", "Default applied")),
    );
  }

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
      format!("{} · {name}", tr(self.lang, "Aplicado", "Applied")),
    );
  }

  fn footer_hints(&self) -> String {
    match self.page {
      DisplayPage::Picker { .. } => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   r Cancelar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   r Cancel   ? Help",
      )
      .into(),
      DisplayPage::Prompt { .. } => tr(
        self.lang,
        "Digite o valor   Enter Confirmar   Esc Cancelar",
        "Type value   Enter Confirm   Esc Cancel",
      )
      .into(),
      DisplayPage::Profiles => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   Enter Novo   r Recarregar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   Enter New   r Reload   ←/Esc Back   ? Help",
      )
      .into(),
      DisplayPage::Detail(_) => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   →/Enter Abrir   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   →/Enter Open   r Refresh   ←/Esc Back   ? Help",
      )
      .into(),
      DisplayPage::Home => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   r Refresh   ←/Esc Back   ? Help",
      )
      .into(),
    }
  }

  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Monitores", "Displays");
    match self.page {
      DisplayPage::Home => root.into(),
      DisplayPage::Detail(index) => {
        let name = self
          .monitors
          .get(index)
          .map(|monitor| monitor.name.clone())
          .or_else(|| self.stale_name(index))
          .unwrap_or_else(|| tr(self.lang, "Monitor", "Monitor").into());
        format!("{root} > {name}")
      }
      DisplayPage::Picker { setting, .. } => {
        format!("{root} > {}", setting_label(self.lang, setting))
      }
      DisplayPage::Profiles => format!("{root} > {}", tr(self.lang, "Perfis", "Profiles")),
      DisplayPage::Prompt { goal } => format!(
        "{root} > {}",
        match goal {
          PromptGoal::Position(_) => tr(self.lang, "Posição", "Position"),
          PromptGoal::Scale(_) => tr(self.lang, "Escala", "Scale"),
          PromptGoal::SdrBrightness(_) => tr(self.lang, "Brilho SDR", "SDR brightness"),
          PromptGoal::SdrSaturation(_) => tr(self.lang, "Saturação SDR", "SDR saturation"),
          PromptGoal::ProfileCreate => tr(self.lang, "Novo perfil", "New profile"),
          PromptGoal::ProfileRename(_) => tr(self.lang, "Renomear perfil", "Rename profile"),
        }
      ),
    }
  }

  fn is_picker(&self) -> bool {
    matches!(self.page, DisplayPage::Picker { .. })
  }

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
        vec![tr(self.lang, "Sem opções disponíveis", "No options available").into()]
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
      let split = Layout::vertical([Constraint::Length(info_height), Constraint::Min(1)]).split(rest);
      (split[0], split[1])
    } else {
      (Rect::new(rest.x, rest.y, rest.width, 0), rest)
    };
    if info_height > 0 {
      let lines: Vec<Line> = std::iter::once(Line::from(""))
        .chain(info.iter().map(|(label, value)| {
          Line::from(vec![
            Span::styled(
              format!(" {label}"),
              Style::new().fg(self.theme.muted),
            ),
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

  fn draw_prompt(&mut self, frame: &mut Frame, area: Rect, goal: PromptGoal) {
    let width = area.width.saturating_sub(6).clamp(34, 76);
    let height = 9u16.min(area.height);
    let x = area.x + (area.width - width).saturating_div(2);
    let y = area.y + (area.height.saturating_sub(height)).saturating_div(2);
    let box_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, box_area);
    let title = match goal {
      PromptGoal::Position(_) => tr(self.lang, "Posição (X,Y)", "Position (X,Y)"),
      PromptGoal::Scale(_) => tr(self.lang, "Escala", "Scale"),
      PromptGoal::SdrBrightness(_) => tr(self.lang, "Brilho SDR", "SDR brightness"),
      PromptGoal::SdrSaturation(_) => tr(self.lang, "Saturação SDR", "SDR saturation"),
      PromptGoal::ProfileCreate => tr(self.lang, "Novo perfil", "New profile"),
      PromptGoal::ProfileRename(_) => tr(self.lang, "Renomear perfil", "Rename profile"),
    };
    let display = if self.prompt_buffer.is_empty() {
      " ".into()
    } else {
      self.prompt_buffer.clone()
    };
    let mut lines = vec![Line::from("")];
    lines.push(Line::from(vec![
      Span::styled("> ", Style::new().fg(self.theme.accent)),
      Span::styled(display, Style::new().fg(self.theme.foreground).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));
    if let Some(error) = &self.prompt_error {
      lines.push(Line::from(vec![Span::styled(
        format!("⚠ {error}"),
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
        tr(
          self.lang,
          "Excluir este perfil?",
          "Delete this profile?"
        ),
        name,
      );
      argvus_tui::components::draw_confirmation(
        frame,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(self.lang, "Excluir perfil", "Delete profile"),
          message: &message,
          confirm_label: tr(self.lang, "Excluir", "Delete"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
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

fn split_buttons(
  frame: &mut Frame,
  body: Rect,
  raw_buttons: &[Button],
) -> (Rect, Option<Rect>) {
  let _ = frame;
  if raw_buttons.is_empty() {
    return (body, None);
  }
  let button_height = argvus_tui::buttons::height(raw_buttons, body.width).min(body.height);
  let split = Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(body);
  (split[0], Some(split[1]))
}

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

fn draw_revert(frame: &mut Frame, area: Rect, theme: &Theme, lang: Lang, revert: &RevertState) {
  let remaining = revert
    .deadline
    .saturating_duration_since(Instant::now())
    .as_secs();
  let message = format!(
    "{} ({}s) · {} {}",
    tr(
      lang,
      "Manter esta configuração? [Enter] = manter",
      "Keep this configuration? [Enter] = keep"
    ),
    remaining,
    tr(
      lang,
      "revertendo ao automático se nenhuma tecla for pressionada",
      "reverting automatically if no key is pressed"
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

fn enabled_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  vec![
    PickerOption {
      label: tr(lang, "Ligar", "Enable").into(),
      args: monitor.enabled_with_current_args(),
      persist: PersistChange::Disabled(false),
      prompt: None,
    },
    PickerOption {
      label: tr(lang, "Desligar", "Disable").into(),
      args: monitor.disabled_args(),
      persist: PersistChange::Disabled(true),
      prompt: None,
    },
  ]
}

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

fn mirror_options(monitor: &Monitor, monitors: &[Monitor], lang: Lang) -> Vec<PickerOption> {
  let mut options = vec![PickerOption {
    label: tr(lang, "Sem espelhamento", "No mirror").into(),
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

fn dpms_options(monitor: &Monitor, lang: Lang) -> Vec<PickerOption> {
  let current_on = monitor.dpms_status != "off";
  vec![
    PickerOption {
      label: format!(
        "{}  {}",
        tr(lang, "Ligar", "On"),
        if current_on {
          tr(lang, "· atual", "· current").into()
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
        tr(lang, "Desligar", "Off"),
        if !current_on {
          tr(lang, "· atual", "· current").into()
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

fn option_is_risky(persist: &PersistChange) -> bool {
  matches!(
    persist,
    PersistChange::Mode(_) | PersistChange::Position(_, _) | PersistChange::Mirror(_) | PersistChange::Disabled(true) | PersistChange::BitDepth(_)
  )
}

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

fn apply_wallpapers_hook() -> Result<(), String> {
  let script = "/usr/bin/argvus-wallpapers-apply";
  if std::path::Path::new(script).exists() {
    let _ = std::process::Command::new(script).status();
  }
  Ok(())
}

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

fn trimmed_rate(rate: f64) -> String {
  let rounded = (rate * 1000.0).round() / 1000.0;
  if rounded.fract() < 0.0005 {
    format!("{}", rounded as u64)
  } else {
    format!("{rounded:.3}")
  }
}

fn rate_of(mode: &str) -> Option<f64> {
  mode.rsplit('@').next()?.parse::<f64>().ok()
}

fn parse_position(buffer: &str) -> Option<(i32, i32)> {
  let (x, y) = buffer.split_once(',')?;
  Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn parse_number(buffer: &str) -> Option<f64> {
  buffer.trim().parse::<f64>().ok().filter(|value| value.is_finite())
}

fn prompt_allowed(goal: PromptGoal, character: char) -> bool {
  match goal {
    PromptGoal::Position(_) => character.is_ascii_digit() || character == 'x' || character == ',' || character == '-',
    PromptGoal::Scale(_)
    | PromptGoal::SdrBrightness(_)
    | PromptGoal::SdrSaturation(_) => character.is_ascii_digit() || character == '.',
    PromptGoal::ProfileCreate | PromptGoal::ProfileRename(_) => !character.is_control(),
  }
}

fn setting_label(lang: Lang, setting: MonitorSetting) -> String {
  match setting {
    MonitorSetting::Resolution => tr(lang, "Resolução", "Resolution").into(),
    MonitorSetting::RefreshRate => tr(lang, "Taxa de atualização", "Refresh rate").into(),
    MonitorSetting::Scale => tr(lang, "Escala", "Scale").into(),
    MonitorSetting::Position => tr(lang, "Posição", "Position").into(),
    MonitorSetting::Primary => tr(lang, "Monitor primário", "Primary monitor").into(),
    MonitorSetting::Enabled => tr(lang, "Monitor ligado", "Monitor enabled").into(),
    MonitorSetting::Mirror => tr(lang, "Espelhamento", "Mirror").into(),
    MonitorSetting::BitDepth => tr(lang, "Profundidade de cor", "Color depth").into(),
    MonitorSetting::Vrr => tr(lang, "VRR", "VRR").into(),
    MonitorSetting::Hdr => tr(lang, "HDR", "HDR").into(),
    MonitorSetting::Dpms => "DPMS".into(),
    MonitorSetting::SdrBrightness => tr(lang, "Brilho SDR", "SDR brightness").into(),
    MonitorSetting::SdrSaturation => tr(lang, "Saturação SDR", "SDR saturation").into(),
    MonitorSetting::Workspaces => tr(lang, "Workspaces", "Workspaces").into(),
    MonitorSetting::Orientation => tr(lang, "Orientação", "Orientation").into(),
  }
}

fn transform_label(lang: Lang, transform: i32) -> String {
  match transform {
    0 => tr(lang, "Normal", "Normal").into(),
    1 => tr(lang, "90°", "90°").into(),
    2 => tr(lang, "180°", "180°").into(),
    3 => tr(lang, "270°", "270°").into(),
    4 => tr(lang, "Espelhado", "Flipped").into(),
    5 => tr(lang, "Espelhado 90°", "Flipped 90°").into(),
    6 => tr(lang, "Espelhado 180°", "Flipped 180°").into(),
    7 => tr(lang, "Espelhado 270°", "Flipped 270°").into(),
    _ => format!("{transform}"),
  }
}

fn vrr_label(lang: Lang, vrr: i32) -> String {
  match vrr {
    0 => tr(lang, "Desativado", "Disabled").into(),
    1 => tr(lang, "Ativado", "Enabled").into(),
    2 => tr(lang, "Somente em tela cheia", "Fullscreen only").into(),
    _ => format!("{vrr}"),
  }
}

fn hdr_label(lang: Lang, hdr: i32) -> String {
  if hdr != 0 {
    tr(lang, "Forçado", "Forced").into()
  } else {
    tr(lang, "Automático", "Auto").into()
  }
}

fn on_off(lang: Lang, on: bool) -> String {
  if on {
    tr(lang, "Sim", "Yes").into()
  } else {
    tr(lang, "Não", "No").into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::{Mode};

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

  fn app_with(job: bool) -> DisplaysApp {
    let mut app = DisplaysApp::new(Lang::En, Theme::load());
    if !job {
      app.job = None;
      app.action = None;
      app.hotplug = None;
    }
    app.monitors = vec![monitor("eDP-1")];
    app
  }

  #[test]
  fn versions_gate_fancy_features() {
    let mut app = app_with(true);
    assert!(!app.vrr_supported() && !app.hdr_supported());
    app.version = (0, 42);
    assert!(app.vrr_supported() && app.hdr_supported());
  }

  #[test]
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
  fn prompt_validation_rejects_bad_positions() {
    assert_eq!(parse_position("1920,0"), Some((1920, 0)));
    assert_eq!(parse_position("0x0"), None);
    assert_eq!(parse_position("abc"), None);
    assert_eq!(parse_number("1.5"), Some(1.5));
    assert_eq!(parse_number("x"), None);
  }

  #[test]
  fn workspace_binding_persist_moves_workspace() {
    let mut config = PersistedConfig::default();
    config.set_workspaces("eDP-1", vec![1, 2]);
    config.set_workspaces("DP-1", vec![3]);
    DisplaysApp::apply_persist(&mut config, "DP-1", &PersistChange::Workspace(2, Some("DP-1".into())));
    assert_eq!(config.workspaces_of("eDP-1"), vec![1]);
    assert_eq!(config.workspaces_of("DP-1"), vec![2, 3]);
    DisplaysApp::apply_persist(&mut config, "DP-1", &PersistChange::Workspace(2, None));
    assert_eq!(config.workspaces_of("DP-1"), vec![3]);
  }

  #[test]
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
    assert!(rule.starts_with("eDP-1, 1920x1080@144, 0x0, 1.25, transform, 1"), "{rule}");
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
    assert!(options.iter().all(|option| option.persist == PersistChange::Primary));
  }

  #[test]
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
    assert!(options[0].label.contains("this monitor"), "{:?}", options[0].label);
    assert!(options[2].label.contains("another monitor"), "{:?}", options[2].label);
    assert!(options[4].label.contains("unbound"), "{:?}", options[4].label);
  }

  #[test]
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