//! Implements terminal UI rendering and interaction in crate `argvus control center appearance`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use crate::{
  backend,
  model::{
    AppearancePage, AppearanceState, ControlPanelCard, ControlPanelCards, CustomTheme, HexColor,
    PromptGoal, THEME_FAMILIES, TaskbarPosition, TaskbarUtilityGroupMode, ThemeCategory,
    WidgetTelemetryBlock, accent_label, normalize_hex_color, theme_family_label,
  },
};
use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  buttons::{Button, ButtonKind},
  components::{StatusKind, StatusMessage},
  page::{list, shell, status},
};
use crossterm::event::KeyCode;
use ratatui::{
  Frame,
  layout::{Constraint, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::Paragraph,
  widgets::{Block, Borders},
};

#[derive(Debug, Clone)]
/// Defines `JobData`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
enum JobData {
  Loaded(AppearancePage, Box<AppearanceState>),
  Archives(Vec<std::path::PathBuf>),
  Action(String),
  ImportReady {
    path: std::path::PathBuf,
    name: String,
    duplicate: bool,
  },
}

/// Executes the `icon_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn icon_label(icon: &'static str, label: impl AsRef<str>) -> String {
  argvus_tui::icons::icon_label(AppConfig::icon(icon), label)
}

fn family_indices(category: ThemeCategory) -> Vec<usize> {
  THEME_FAMILIES
    .iter()
    .enumerate()
    .filter_map(|(index, (family_id, _))| category.contains_family(family_id).then_some(index))
    .collect()
}

fn family_index(category: ThemeCategory, selected: usize) -> Option<usize> {
  family_indices(category).get(selected).copied()
}

/// Represents `AppearanceApp`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct AppearanceApp {
  pub page: AppearancePage,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
  state: AppearanceState,
  loaded: bool,
  status_loading: bool,
  selected: usize,
  prompt_buffer: String,
  prompt_error: Option<String>,
  prompt_back: Option<AppearancePage>,
  job: Option<JobHandle<JobData>>,
  refreshed: Vec<(AppearancePage, std::time::Instant)>,
  action: Option<JobHandle<JobData>>,
  reload_requested: bool,
  control_panel_draft: Option<ControlPanelCards>,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  delete_theme: Option<CustomTheme>,
  import_archives: Vec<std::path::PathBuf>,
  pending_import: Option<std::path::PathBuf>,
  pending_import_name: Option<String>,
  manager: JobManager,
}
impl AppearanceApp {
  /// Executes the `reload` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn reload(&mut self) {
    self.ensure_page();
  }

  fn ensure_page(&mut self) {
    let lifetime = if self.page == AppearancePage::ControlPanel {
      60
    } else {
      30
    };
    if !self
      .refreshed
      .iter()
      .any(|(page, at)| *page == self.page && at.elapsed().as_secs() < lifetime)
    {
      self.refresh();
    }
  }

  pub fn paste(&mut self, text: &str) {
    if self.page != AppearancePage::AccentEdit {
      return;
    }
    let value = text.lines().next().unwrap_or_default().trim();
    if let Some(color) = normalize_hex_color(value) {
      self.prompt_buffer = color;
      self.prompt_error = None;
    } else {
      self.prompt_error = Some(tr(self.lang, "control_center.accent_invalid").into());
    }
  }
  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn new(lang: Lang, theme: Theme) -> Self {
    Self {
      page: AppearancePage::Home,
      lang,
      theme,
      status: None,
      state: AppearanceState::default(),
      loaded: false,
      status_loading: false,
      selected: 0,
      prompt_buffer: String::new(),
      prompt_error: None,
      prompt_back: None,
      job: None,
      refreshed: Vec::new(),
      action: None,
      reload_requested: false,
      control_panel_draft: None,
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      delete_theme: None,
      import_archives: Vec::new(),
      pending_import: None,
      pending_import_name: None,
      manager: JobManager::default(),
    }
  }
  /// Executes the `refresh` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn refresh(&mut self) {
    if self.job.is_none() && self.action.is_none() {
      self.status_loading = true;
      let page = self.page;
      let state = self.state.clone();
      self.job = Some(self.manager.spawn(move |_| {
        if page == AppearancePage::ThemeImport {
          Ok(JobData::Archives(backend::list_import_archives()))
        } else {
          Ok(JobData::Loaded(
            page,
            Box::new(backend::load_page(page, state)),
          ))
        }
      }));
    }
  }
  /// Executes the `poll` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(JobData::Loaded(page, state)) => {
          self.refreshed.retain(|(old, _)| *old != page);
          self.refreshed.push((page, std::time::Instant::now()));
          self.state = *state;
          self.loaded = true;
          self.status_loading = false;
          if page == AppearancePage::ControlPanel && self.page == page {
            self.control_panel_draft = Some(self.state.control_panel_cards.clone());
          }
          if self.page != page {
            self.refresh();
          }
        }
        Ok(JobData::Archives(paths)) => {
          self.import_archives = paths;
          self.status_loading = false;
        }
        Err(error) => {
          self.status_loading = false;
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: format!("{} {error}", tr(self.lang, "control_center.error")),
          });
        }
        Ok(JobData::Action(_)) => {}
        Ok(JobData::ImportReady { .. }) => {}
      }
      changed = true;
    }
    if let Some(action) = &self.action
      && let JobState::Finished(result) = action.try_state()
    {
      self.action = None;
      if self.reload_requested {
        self.reload_requested = false;
        self.refreshed.clear();
        self.refresh();
      }
      match result {
        Ok(JobData::Action(text)) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: text.clone(),
          })
        }
        Ok(JobData::ImportReady {
          path,
          name,
          duplicate,
        }) => {
          self.pending_import = Some(path);
          self.pending_import_name = Some(name);
          if duplicate {
            self.status = None;
            self.go(AppearancePage::ThemeImportConfirm);
          } else {
            self.start_pending_import();
            self.go(AppearancePage::Themes);
          }
        }
        Err(error) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: format!("{} {error}", tr(self.lang, "control_center.error")),
          })
        }
        Ok(_) => {}
      }
      changed = true;
    }
    changed
  }

  fn start_pending_import(&mut self) {
    let Some(path) = self.pending_import.take() else {
      return;
    };
    self.apply(
      tr(self.lang, "control_center.theme_profile_imported").into(),
      move || backend::import_theme_profile(&path).map(|_| ()),
    );
  }
  /// Applies the `apply` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply(&mut self, message: String, task: impl FnOnce() -> Result<(), String> + Send + 'static) {
    if self.action.is_some() {
      return;
    }
    self.reload_requested = true;
    self.action = Some(self.manager.spawn(move |_| {
      task()?;
      Ok(JobData::Action(message))
    }));
  }

  fn apply_result(&mut self, task: impl FnOnce() -> Result<String, String> + Send + 'static) {
    if self.action.is_some() {
      return;
    }
    self.reload_requested = true;
    self.action = Some(self.manager.spawn(move |_| {
      let text = task()?;
      Ok(JobData::Action(text))
    }));
  }
  /// Executes the `go` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn go(&mut self, page: AppearancePage) {
    if self.page != page
      && let Some(job) = self.job.take()
    {
      job.cancel();
      self.status_loading = false;
    }
    if self.page == AppearancePage::ControlPanel && page != AppearancePage::ControlPanel {
      self.control_panel_draft = None;
    }
    self.page = page;
    self.selected = 0;
    self.on_buttons = false;
    self.button_from = None;
    if page == AppearancePage::ControlPanel {
      self.control_panel_draft = Some(self.state.control_panel_cards.clone());
    }
    self.ensure_page();
  }

  /// Applies all pending Control Panel checkbox changes with one panel reload.
  fn apply_control_panel_changes(&mut self) {
    let Some(draft) = self.control_panel_draft.clone() else {
      return;
    };
    let changes = ControlPanelCard::ALL
      .into_iter()
      .filter_map(|card| {
        (draft.enabled(card) != self.state.control_panel_cards.enabled(card))
          .then_some((card, draft.enabled(card)))
      })
      .collect::<Vec<_>>();
    if changes.is_empty() {
      return;
    }
    self.control_panel_draft = None;
    self.apply(
      tr(self.lang, "control_center.control_panel_changed").into(),
      move || backend::set_control_panel_cards(changes),
    );
  }
  /// Applies the `toggle` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn toggle(&mut self) {
    let value = if self.state.rounded { "0" } else { "1" };
    let enable = !self.state.rounded;
    self.apply(
      tr(self.lang, "control_center.border_settings_applied").into(),
      move || {
        backend::set_border("rounded", value).and_then(|_| {
          if enable {
            backend::set_border("rounding", "2")
          } else {
            Ok(())
          }
        })
      },
    );
  }
  /// Executes the `pick` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn pick(&mut self) {
    match self.page {
      AppearancePage::Themes => match self.selected {
        0 => self.go(AppearancePage::ThemeFamilies {
          category: ThemeCategory::Dark,
        }),
        1 => self.go(AppearancePage::ThemeFamilies {
          category: ThemeCategory::Light,
        }),
        2 => self.go(AppearancePage::CustomThemes),
        3 => self.open_prompt(PromptGoal::ExportProfile),
        4 => self.go(AppearancePage::ThemeImport),
        _ => {}
      },
      AppearancePage::ThemeFamilies { category } => {
        let Some(family) = family_index(category, self.selected) else {
          return;
        };
        self.go(AppearancePage::ThemeModes { family });
      }
      AppearancePage::CustomThemes => {
        let Some(custom) = self.state.custom_themes.get(self.selected).cloned() else {
          return;
        };
        let lang = self.lang;
        self.apply_result(move || {
          backend::apply_custom_theme(&custom).map(|report| {
            if report.wallpaper_missing {
              format!(
                "{}: {}",
                tr(lang, "control_center.theme_profile_applied"),
                tr(lang, "control_center.theme_profile_wallpaper_missing")
              )
            } else {
              tr(lang, "control_center.theme_profile_applied").to_string()
            }
          })
        });
      }
      AppearancePage::ThemeImport => {
        if let Some(path) = self.import_archives.get(self.selected).cloned() {
          self.status = Some(StatusMessage {
            kind: StatusKind::Info,
            text: tr(self.lang, "control_center.theme_profile_importing").into(),
          });
          self.action = Some(self.manager.spawn(move |_| {
            let (name, duplicate) = backend::inspect_theme_profile(&path)?;
            Ok(JobData::ImportReady {
              path,
              name,
              duplicate,
            })
          }));
        }
      }
      AppearancePage::ThemeImportConfirm => {
        if self.selected == 0 {
          self.start_pending_import();
          self.go(AppearancePage::Themes);
        } else {
          self.pending_import = None;
          self.pending_import_name = None;
          self.go(AppearancePage::Themes);
        }
      }
      AppearancePage::ThemeDeleteConfirm => {
        if self.selected == 0 {
          if let Some(theme) = self.delete_theme.clone() {
            self.apply(
              tr(self.lang, "control_center.theme_profile_deleted").into(),
              move || backend::delete_custom_theme(&theme),
            );
          }
          self.go(AppearancePage::Themes);
        } else {
          self.go(AppearancePage::Themes);
        }
      }
      AppearancePage::ThemeModes { family } => {
        if let Some((base, _)) = THEME_FAMILIES.get(family) {
          let name = if self.selected == 0 {
            (*base).to_string()
          } else {
            format!("{base}-float")
          };
          self.apply(
            tr(self.lang, "control_center.theme_applied").into(),
            move || backend::set_theme(&name),
          );
        }
      }
      AppearancePage::Accents => {
        if self.selected == 0 {
          self.page = AppearancePage::AccentEdit;
          self.prompt_buffer = self.state.accent.clone();
          self.prompt_error = None;
        } else {
          self.apply(tr(self.lang, "control_center.accent_reset").into(), || {
            backend::set_accent("--theme-default")
          });
        }
      }
      AppearancePage::Wallpapers => {
        if self.selected == 0 {
          self.apply(
            tr(self.lang, "control_center.wallpaper_chooser_opened").into(),
            backend::choose_wallpaper,
          );
        } else if let Some(name) = self.state.wallpapers.get(self.selected - 1) {
          let name = name.clone();
          self.apply(
            tr(self.lang, "control_center.wallpaper_applied").into(),
            move || backend::set_wallpaper(&name),
          );
        }
      }
      AppearancePage::TaskbarPosition => {
        let position = if self.selected == 0 { "top" } else { "bottom" };
        self.apply(
          tr(self.lang, "control_center.taskbar_position_changed").into(),
          move || backend::set_waybar_position(position),
        );
      }
      AppearancePage::Taskbar => {
        let mode = if self.selected == 0 {
          TaskbarUtilityGroupMode::Auto
        } else {
          TaskbarUtilityGroupMode::AlwaysExpanded
        };
        self.apply(
          tr(self.lang, "control_center.taskbar_changed").into(),
          move || backend::set_taskbar_utility_group(mode),
        );
      }
      AppearancePage::WidgetTelemetry => {
        if self.selected == 0 {
          self.apply_toggle_telemetry();
        } else if let Some(block) = self
          .selected
          .checked_sub(2)
          .and_then(|index| WidgetTelemetryBlock::ALL.get(index))
          .copied()
        {
          let enabled = !self.state.widget_telemetry_blocks.enabled(block);
          self.apply(
            tr(self.lang, "control_center.widget_telemetry_block_changed").into(),
            move || backend::set_telemetry_block(block, enabled),
          );
        }
      }
      AppearancePage::ControlPanel => {
        if let Some(card) = self.control_panel_cards().get(self.selected).copied() {
          let draft = self
            .control_panel_draft
            .get_or_insert_with(|| self.state.control_panel_cards.clone());
          draft.set(card, !draft.enabled(card));
        }
      }
      AppearancePage::Effects if self.selected == 0 => self.apply_toggle_animations(),
      AppearancePage::Effects if self.selected == 1 => self.apply_toggle_transparency(),
      AppearancePage::GeneralBorders if self.selected == 0 => self.toggle(),
      AppearancePage::TaskbarSpaces => self.open_prompt(
        [
          PromptGoal::WaybarTop,
          PromptGoal::WaybarLeft,
          PromptGoal::WaybarRight,
          PromptGoal::WaybarBottom,
        ][self.selected],
      ),
      AppearancePage::WindowSpaces => self.open_prompt(
        [
          PromptGoal::GapsIn,
          PromptGoal::GapsOutTop,
          PromptGoal::GapsOutLeft,
          PromptGoal::GapsOutRight,
          PromptGoal::GapsOutBottom,
        ][self.selected],
      ),
      AppearancePage::GeneralBorders if self.selected == 1 && self.state.rounded => {
        self.open_prompt(PromptGoal::Rounding)
      }
      AppearancePage::EdgeThickness => self.open_prompt(PromptGoal::Thickness),
      _ => {}
    }
  }
  /// Executes the `open_prompt` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn open_prompt(&mut self, goal: PromptGoal) {
    self.prompt_back = Some(self.page);
    self.page = AppearancePage::Prompt { goal };
    self.prompt_buffer.clear();
    self.prompt_error = None;
  }
  /// Executes the `prompt_key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn prompt_key(&mut self, key: KeyCode) -> bool {
    if self.page == AppearancePage::AccentEdit {
      match key {
        KeyCode::Enter if self.action.is_none() => {
          if let Some(color) = normalize_hex_color(&self.prompt_buffer) {
            self.apply(
              tr(self.lang, "control_center.accent_applied").into(),
              move || backend::set_accent(&color),
            );
            self.go(AppearancePage::Accents);
          } else {
            self.prompt_error = Some(tr(self.lang, "control_center.accent_invalid").into());
          }
        }
        KeyCode::Esc | KeyCode::Left => self.go(AppearancePage::Accents),
        KeyCode::Backspace | KeyCode::Delete => {
          self.prompt_buffer.pop();
          self.prompt_error = None;
        }
        KeyCode::Char('#') if self.prompt_buffer.is_empty() => self.prompt_buffer.push('#'),
        KeyCode::Char(c)
          if c.is_ascii_hexdigit() && self.prompt_buffer.trim_start_matches('#').len() < 6 =>
        {
          self.prompt_buffer.push(c.to_ascii_uppercase());
          self.prompt_error = None;
        }
        _ => {}
      }
      return false;
    }
    match key {
      KeyCode::Enter if self.action.is_none() => {
        let AppearancePage::Prompt { goal } = self.page else {
          return false;
        };
        let value = self.prompt_buffer.trim().to_string();
        if matches!(goal, PromptGoal::ExportProfile) {
          let back = self.prompt_back.take().unwrap_or(AppearancePage::Themes);
          if value.is_empty() {
            self.prompt_error =
              Some(tr(self.lang, "control_center.theme_profile_invalid_name").into());
            self.prompt_back = Some(back);
            return false;
          }
          let message = tr(self.lang, "control_center.theme_profile_exported").to_string();
          self.apply_result(move || {
            backend::export_theme_profile(&value)
              .map(|path| format!("{message}: {}", path.display()))
          });
          self.go(back);
          return false;
        }
        let (min, max) = goal.range();
        let valid = value
          .parse::<i32>()
          .ok()
          .is_some_and(|v| (min..=max).contains(&v));
        if !valid {
          self.prompt_error = Some(format!(
            "{} ({}–{})",
            tr(self.lang, "control_center.enter_a_valid_integer"),
            min,
            max
          ));
          return false;
        }
        let back = self.prompt_back.take().unwrap_or(AppearancePage::Home);
        let key_name = goal.key();
        let message_key = if matches!(goal, PromptGoal::Rounding) {
          "control_center.border_settings_applied"
        } else if matches!(goal, PromptGoal::Thickness) {
          "control_center.thickness_applied"
        } else {
          "control_center.spacing_applied"
        };
        self.apply(tr(self.lang, message_key).into(), move || match goal {
          PromptGoal::Rounding | PromptGoal::Thickness => backend::set_border(key_name, &value),
          _ => backend::set_spacing(key_name, &value),
        });
        self.go(back);
        false
      }
      KeyCode::Esc | KeyCode::Left => {
        let back = self.prompt_back.take().unwrap_or(AppearancePage::Home);
        self.go(back);
        false
      }
      KeyCode::Char(c)
        if matches!(
          self.page,
          AppearancePage::Prompt {
            goal: PromptGoal::ExportProfile | PromptGoal::ImportProfile
          }
        ) && !c.is_control()
          && self.prompt_buffer.len() < 512 =>
      {
        self.prompt_buffer.push(c);
        false
      }
      KeyCode::Char(c) if c.is_ascii_digit() && self.prompt_buffer.len() < 3 => {
        self.prompt_buffer.push(c);
        false
      }
      KeyCode::Backspace => {
        self.prompt_buffer.pop();
        false
      }
      _ => false,
    }
  }
  /// Processes `handle` in this module's event flow. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.prompt_back.is_some() || self.page == AppearancePage::AccentEdit {
      return self.prompt_key(key);
    }
    if matches!(key, KeyCode::Esc | KeyCode::Left) {
      return self.back();
    }
    if (self.job.is_some() || self.action.is_some())
      && !matches!(
        key,
        KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
      )
      && !(key == KeyCode::Enter && self.page == AppearancePage::Home)
    {
      return false;
    }
    if matches!(
      self.page,
      AppearancePage::Themes | AppearancePage::CustomThemes
    ) {
      if key == KeyCode::Char('e') {
        self.open_prompt(PromptGoal::ExportProfile);
        return false;
      }
      if key == KeyCode::Char('i') {
        self.go(AppearancePage::ThemeImport);
        return false;
      }
      if key == KeyCode::Char('d')
        && self.page == AppearancePage::CustomThemes
        && self.selected < self.state.custom_themes.len()
      {
        if let Some(theme) = self.state.custom_themes.get(self.selected).cloned() {
          self.delete_theme = Some(theme);
          self.go(AppearancePage::ThemeDeleteConfirm);
        }
        return false;
      }
    }
    if self.page == AppearancePage::ControlPanel && !self.buttons().is_empty() {
      if self.on_buttons {
        match key {
          KeyCode::Tab => self.toggle_buttons(false),
          KeyCode::BackTab => self.toggle_buttons(true),
          KeyCode::Left | KeyCode::Char('h') => self.move_button(-1),
          KeyCode::Right | KeyCode::Char('l') => self.move_button(1),
          KeyCode::Enter | KeyCode::Char(' ') => self.activate_button(),
          KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {}
          _ => {}
        }
        return false;
      }
      if matches!(key, KeyCode::Tab | KeyCode::BackTab) {
        self.toggle_buttons(key == KeyCode::BackTab);
        return false;
      }
    }
    if matches!(key, KeyCode::Esc | KeyCode::Left) {
      return self.back();
    }
    let len = self.selection_len();
    match key {
      KeyCode::Char('r') => self.refresh(),
      KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
      KeyCode::Down | KeyCode::Char('j') => {
        self.selected = (self.selected + 1).min(len.saturating_sub(1))
      }
      KeyCode::Home => self.selected = 0,
      KeyCode::End => self.selected = len.saturating_sub(1),
      KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => self.open_or_pick(),
      _ => {}
    }
    false
  }
  /// Executes the `back` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn back(&mut self) -> bool {
    if self.page == AppearancePage::Home {
      return true;
    }
    match self.page {
      AppearancePage::Home => unreachable!(),
      AppearancePage::Themes
      | AppearancePage::Wallpapers
      | AppearancePage::Accents
      | AppearancePage::Effects
      | AppearancePage::SpacesBordersPosition
      | AppearancePage::Taskbar
      | AppearancePage::WidgetTelemetry
      | AppearancePage::ControlPanel => {
        self.go(AppearancePage::Home);
      }
      AppearancePage::ThemeFamilies { .. } | AppearancePage::CustomThemes => {
        self.go(AppearancePage::Themes);
      }
      AppearancePage::ThemeModes { family } => {
        let category = THEME_FAMILIES
          .get(family)
          .and_then(|(family_id, _)| ThemeCategory::for_family_id(family_id))
          .unwrap_or(ThemeCategory::Dark);
        self.go(AppearancePage::ThemeFamilies { category });
      }
      AppearancePage::ThemeImport
      | AppearancePage::ThemeImportConfirm
      | AppearancePage::ThemeDeleteConfirm => {
        self.go(AppearancePage::Themes);
      }
      AppearancePage::TaskbarPosition
      | AppearancePage::TaskbarSpaces
      | AppearancePage::WindowSpaces
      | AppearancePage::GeneralBorders
      | AppearancePage::EdgeThickness => {
        self.go(AppearancePage::SpacesBordersPosition);
      }
      AppearancePage::AccentEdit => self.go(AppearancePage::Accents),
      AppearancePage::Prompt { .. } => {}
    }
    false
  }
  /// Executes the `open_or_pick` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn open_or_pick(&mut self) {
    match self.page {
      AppearancePage::Home => match self.selected {
        0 => self.go(AppearancePage::Themes),
        1 => self.go(AppearancePage::Accents),
        2 => self.go(AppearancePage::Wallpapers),
        3 => self.go(AppearancePage::SpacesBordersPosition),
        4 => self.go(AppearancePage::Taskbar),
        5 => self.go(AppearancePage::Effects),
        6 => self.go(AppearancePage::WidgetTelemetry),
        7 => self.go(AppearancePage::ControlPanel),
        _ => {}
      },
      AppearancePage::SpacesBordersPosition => match self.selected {
        0 => self.go(AppearancePage::TaskbarPosition),
        1 => self.go(AppearancePage::TaskbarSpaces),
        2 => self.go(AppearancePage::WindowSpaces),
        3 => self.go(AppearancePage::GeneralBorders),
        4 => self.go(AppearancePage::EdgeThickness),
        _ => {}
      },
      AppearancePage::Themes
      | AppearancePage::ThemeFamilies { .. }
      | AppearancePage::CustomThemes
      | AppearancePage::ThemeModes { .. }
      | AppearancePage::Wallpapers
      | AppearancePage::Accents
      | AppearancePage::Effects
      | AppearancePage::TaskbarPosition
      | AppearancePage::Taskbar
      | AppearancePage::WidgetTelemetry
      | AppearancePage::ControlPanel
      | AppearancePage::TaskbarSpaces
      | AppearancePage::WindowSpaces
      | AppearancePage::GeneralBorders
      | AppearancePage::EdgeThickness => self.pick(),
      AppearancePage::ThemeImport
      | AppearancePage::ThemeImportConfirm
      | AppearancePage::ThemeDeleteConfirm => self.pick(),
      AppearancePage::AccentEdit => {}
      AppearancePage::Prompt { .. } => {}
    }
  }
  /// Applies the `apply_toggle_animations` operation while preserving the persistence and local-update contract.
  fn apply_toggle_animations(&mut self) {
    let value = !self.state.animations;
    self.apply(
      tr(self.lang, "control_center.animations_applied").into(),
      move || backend::set_animations(value),
    );
  }
  /// Applies the `apply_toggle_transparency` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_toggle_transparency(&mut self) {
    let value = !self.state.transparency;
    self.apply(
      tr(self.lang, "control_center.transparency_applied").into(),
      move || backend::set_transparency(value),
    );
  }
  /// Applies the `apply_toggle_telemetry` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_toggle_telemetry(&mut self) {
    let value = !self.state.widget_telemetry;
    self.apply(
      tr(self.lang, "control_center.widget_telemetry_changed").into(),
      move || backend::set_telemetry(value),
    );
  }
  /// Executes the `selection_len` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn selection_len(&self) -> usize {
    match self.page {
      AppearancePage::Home => 8,
      AppearancePage::Themes => 5,
      AppearancePage::ThemeFamilies { category } => family_indices(category).len(),
      AppearancePage::CustomThemes => self.state.custom_themes.len(),
      AppearancePage::Accents => 2,
      AppearancePage::AccentEdit => 0,
      AppearancePage::Effects => 2,
      AppearancePage::ThemeModes { .. }
      | AppearancePage::TaskbarPosition
      | AppearancePage::Taskbar => 2,
      AppearancePage::WidgetTelemetry => 2 + WidgetTelemetryBlock::ALL.len(),
      AppearancePage::ControlPanel => self.control_panel_cards().len(),
      AppearancePage::Wallpapers => self.state.wallpapers.len() + 1,
      AppearancePage::SpacesBordersPosition => 5,
      AppearancePage::TaskbarSpaces => 4,
      AppearancePage::WindowSpaces => 5,
      AppearancePage::GeneralBorders => 2,
      AppearancePage::EdgeThickness => 1,
      AppearancePage::Prompt { .. } => 0,
      AppearancePage::ThemeImport => self.import_archives.len(),
      AppearancePage::ThemeImportConfirm => 2,
      AppearancePage::ThemeDeleteConfirm => 2,
    }
  }
  /// Executes the `home_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn home_rows(&self) -> Vec<String> {
    vec![
      format!(
        "{} · {} [{}]",
        icon_label(
          argvus_tui::icons::PALETTE,
          tr(self.lang, "control_center.theme")
        ),
        theme_family_label(&self.state.theme),
        if self.state.is_float_theme() {
          tr(self.lang, "control_center.theme_mode_float")
        } else {
          tr(self.lang, "control_center.theme_mode_sticky")
        }
      ),
      format!(
        "{} · {}",
        icon_label(
          argvus_tui::icons::PALETTE,
          tr(self.lang, "control_center.highlight_color")
        ),
        accent_label(&self.state.accent)
      ),
      format!(
        "{} · {}",
        icon_label(
          argvus_tui::icons::IMAGE,
          tr(self.lang, "control_center.wallpaper")
        ),
        self
          .state
          .wallpaper_active
          .clone()
          .unwrap_or_else(|| tr(self.lang, "control_center.none").to_string())
      ),
      icon_label(
        argvus_tui::icons::STORAGE,
        tr(self.lang, "control_center.spaces_borders_position"),
      ),
      icon_label(
        argvus_tui::icons::STORAGE,
        tr(self.lang, "control_center.taskbar"),
      ),
      icon_label(
        argvus_tui::icons::SUCCESS,
        tr(self.lang, "control_center.effects"),
      ),
      icon_label(
        argvus_tui::icons::WIDGET,
        tr(self.lang, "control_center.widget_telemetry"),
      ),
      icon_label(
        argvus_tui::icons::WIDGET,
        tr(self.lang, "control_center.control_panel"),
      ),
    ]
  }
  /// Executes the `prompt_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn prompt_label(&self, goal: PromptGoal) -> &'static str {
    match goal {
      PromptGoal::ExportProfile => "control_center.theme_profile_export_name",
      PromptGoal::ImportProfile => "control_center.theme_profile_import_path",
      PromptGoal::WaybarTop => "control_center.top",
      PromptGoal::WaybarLeft => "control_center.left",
      PromptGoal::WaybarRight => "control_center.right",
      PromptGoal::WaybarBottom => "control_center.bottom",
      PromptGoal::GapsIn => "control_center.inner_gap",
      PromptGoal::GapsOutTop => "control_center.outer_gap_top",
      PromptGoal::GapsOutLeft => "control_center.outer_gap_left",
      PromptGoal::GapsOutRight => "control_center.outer_gap_right",
      PromptGoal::GapsOutBottom => "control_center.outer_gap_bottom",
      PromptGoal::Rounding => "control_center.rounding",
      PromptGoal::Thickness => "control_center.thickness",
    }
  }
  /// Executes the `breadcrumb` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "control_center.appearance");
    match self.page {
      AppearancePage::Home => root.into(),
      AppearancePage::Themes => format!("{root} › {}", tr(self.lang, "control_center.themes")),
      AppearancePage::ThemeImport => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.themes"),
        tr(self.lang, "control_center.theme_profile_import")
      ),
      AppearancePage::ThemeImportConfirm => {
        tr(self.lang, "control_center.theme_profile_duplicate_title").into()
      }
      AppearancePage::ThemeDeleteConfirm => {
        tr(self.lang, "control_center.theme_profile_delete_title").into()
      }
      AppearancePage::ThemeFamilies { category } => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.themes"),
        tr(self.lang, category.label_key())
      ),
      AppearancePage::CustomThemes => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.themes"),
        tr(self.lang, "control_center.theme_profile_custom")
      ),
      AppearancePage::ThemeModes { family } => {
        let label = THEME_FAMILIES
          .get(family)
          .map(|(_, label)| *label)
          .unwrap_or_default();
        format!(
          "{root} › {} › {label}",
          tr(self.lang, "control_center.themes")
        )
      }
      AppearancePage::Wallpapers => {
        format!("{root} › {}", tr(self.lang, "control_center.wallpapers"))
      }
      AppearancePage::Accents => format!(
        "{root} › {}",
        tr(self.lang, "control_center.highlight_color")
      ),
      AppearancePage::AccentEdit => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.highlight_color"),
        tr(self.lang, "control_center.edit_highlight_color")
      ),
      AppearancePage::Effects => {
        format!("{root} › {}", tr(self.lang, "control_center.effects"))
      }
      AppearancePage::SpacesBordersPosition => format!(
        "{root} › {}",
        tr(self.lang, "control_center.spaces_borders_position")
      ),
      AppearancePage::TaskbarPosition => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.spaces_borders_position"),
        tr(self.lang, "control_center.taskbar_position")
      ),
      AppearancePage::TaskbarSpaces => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.spaces_borders_position"),
        tr(self.lang, "control_center.taskbar_spaces")
      ),
      AppearancePage::Taskbar => {
        format!("{root} › {}", tr(self.lang, "control_center.taskbar"))
      }
      AppearancePage::WidgetTelemetry => format!(
        "{root} › {}",
        tr(self.lang, "control_center.widget_telemetry")
      ),
      AppearancePage::ControlPanel => {
        format!("{root} › {}", tr(self.lang, "control_center.control_panel"))
      }
      AppearancePage::WindowSpaces => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.spaces_borders_position"),
        tr(self.lang, "control_center.window_spaces")
      ),
      AppearancePage::GeneralBorders => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.spaces_borders_position"),
        tr(self.lang, "control_center.general_borders")
      ),
      AppearancePage::EdgeThickness => format!(
        "{root} › {} › {}",
        tr(self.lang, "control_center.spaces_borders_position"),
        tr(self.lang, "control_center.edge_thickness")
      ),
      AppearancePage::Prompt { goal } => format!(
        "{} › {}",
        self.breadcrumb_for_prompt(goal),
        tr(self.lang, self.prompt_label(goal))
      ),
    }
  }
  /// Executes the `breadcrumb_for_prompt` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn breadcrumb_for_prompt(&self, _goal: PromptGoal) -> String {
    let root = tr(self.lang, "control_center.appearance");
    let section = match self.prompt_back.unwrap_or(AppearancePage::Home) {
      AppearancePage::TaskbarSpaces => tr(self.lang, "control_center.taskbar_spaces"),
      AppearancePage::WindowSpaces => tr(self.lang, "control_center.window_spaces"),
      AppearancePage::GeneralBorders => tr(self.lang, "control_center.general_borders"),
      AppearancePage::EdgeThickness => tr(self.lang, "control_center.edge_thickness"),
      _ => "",
    };
    format!(
      "{root} › {} › {section}",
      tr(self.lang, "control_center.spaces_borders_position")
    )
  }
  /// Executes the `rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn rows(&self) -> Vec<String> {
    match self.page {
      AppearancePage::Home => self.home_rows(),
      AppearancePage::Themes => [
        (
          ThemeCategory::Dark,
          tr(self.lang, ThemeCategory::Dark.label_key()),
        ),
        (
          ThemeCategory::Light,
          tr(self.lang, ThemeCategory::Light.label_key()),
        ),
      ]
      .into_iter()
      .map(|(_, label)| format!("{label} >"))
      .chain(std::iter::once(format!(
        "{} >",
        tr(self.lang, "control_center.theme_profile_custom")
      )))
      .chain([
        icon_label(
          argvus_tui::icons::STORAGE,
          tr(self.lang, "control_center.theme_profile_export"),
        ),
        icon_label(
          argvus_tui::icons::STORAGE,
          tr(self.lang, "control_center.theme_profile_import"),
        ),
      ])
      .collect(),
      AppearancePage::ThemeFamilies { category } => {
        let current = self
          .state
          .theme
          .strip_suffix("-float")
          .unwrap_or(&self.state.theme);
        family_indices(category)
          .into_iter()
          .map(|family_index| {
            let (name, label) = THEME_FAMILIES[family_index];
            let is_current = self.state.active_custom_theme.is_none() && current == name;
            let suffix = if is_current {
              format!(" · {}", tr(self.lang, "control_center.current"))
            } else {
              String::new()
            };
            format!("{label}{suffix} >")
          })
          .collect()
      }
      AppearancePage::CustomThemes => self
        .state
        .custom_themes
        .iter()
        .map(|theme| {
          let suffix = if self.state.active_custom_theme.as_deref() == Some(theme.id.as_str()) {
            format!(" · {}", tr(self.lang, "control_center.current"))
          } else {
            String::new()
          };
          format!("{}{}", theme.name, suffix)
        })
        .collect(),
      AppearancePage::ThemeImport => self
        .import_archives
        .iter()
        .map(|path| {
          path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
        })
        .collect(),
      AppearancePage::ThemeDeleteConfirm => vec![
        tr(self.lang, "control_center.theme_profile_delete").into(),
        tr(self.lang, "control_center.cancel").into(),
      ],
      AppearancePage::ThemeImportConfirm => vec![
        tr(self.lang, "control_center.theme_profile_duplicate_replace").into(),
        tr(self.lang, "control_center.cancel").into(),
      ],
      AppearancePage::ThemeModes { .. } => [
        (
          argvus_tui::icons::FOLDER,
          "control_center.theme_mode_sticky",
          !self.state.is_float_theme(),
        ),
        (
          "🪟",
          "control_center.theme_mode_float",
          self.state.is_float_theme(),
        ),
      ]
      .into_iter()
      .map(|(icon, key, current)| {
        let suffix = if current {
          format!(" · {}", tr(self.lang, "control_center.current"))
        } else {
          String::new()
        };
        format!("{}{}", icon_label(icon, tr(self.lang, key)), suffix)
      })
      .collect(),
      AppearancePage::Wallpapers => std::iter::once(icon_label(
        "📂",
        tr(self.lang, "control_center.choose_image_from_home"),
      ))
      .chain(self.state.wallpapers.iter().map(|value| {
        if self.state.wallpaper_active.as_deref() == Some(value.as_str()) {
          format!("{value} · {}", tr(self.lang, "control_center.current"))
        } else {
          value.to_string()
        }
      }))
      .collect(),
      AppearancePage::Accents => vec![
        format!(
          "{}: {}",
          tr(self.lang, "control_center.edit_highlight_color"),
          self.state.accent
        ),
        tr(self.lang, "control_center.reset_to_theme_default").into(),
      ],
      AppearancePage::Effects => vec![
        format!(
          "[{}] {} · {}",
          if self.state.animations { "x" } else { " " },
          icon_label(
            argvus_tui::icons::SUCCESS,
            tr(self.lang, "control_center.animations")
          ),
          if self.state.animations {
            tr(self.lang, "control_center.enabled")
          } else {
            tr(self.lang, "control_center.disabled")
          }
        ),
        format!(
          "[{}] {} · {}",
          if self.state.transparency { "x" } else { " " },
          icon_label(
            argvus_tui::icons::SUCCESS,
            tr(self.lang, "control_center.transparency")
          ),
          if self.state.transparency {
            tr(self.lang, "control_center.enabled")
          } else {
            tr(self.lang, "control_center.disabled")
          }
        ),
      ],
      AppearancePage::SpacesBordersPosition => vec![
        icon_label(
          argvus_tui::icons::INFO,
          tr(self.lang, "control_center.taskbar_position"),
        ),
        icon_label(
          argvus_tui::icons::STORAGE,
          tr(self.lang, "control_center.taskbar_spaces"),
        ),
        icon_label(
          argvus_tui::icons::STORAGE,
          tr(self.lang, "control_center.window_spaces"),
        ),
        icon_label(
          argvus_tui::icons::INFO,
          tr(self.lang, "control_center.general_borders"),
        ),
        icon_label(
          argvus_tui::icons::INFO,
          tr(self.lang, "control_center.edge_thickness"),
        ),
      ],
      AppearancePage::Taskbar => [
        (
          "control_center.taskbar_utility_group_auto",
          self.state.taskbar_utility_group == TaskbarUtilityGroupMode::Auto,
        ),
        (
          "control_center.taskbar_utility_group_always_expanded",
          self.state.taskbar_utility_group == TaskbarUtilityGroupMode::AlwaysExpanded,
        ),
      ]
      .into_iter()
      .map(|(key, current)| {
        let suffix = if current {
          format!(" · {}", tr(self.lang, "control_center.current"))
        } else {
          String::new()
        };
        format!("{}{}", tr(self.lang, key), suffix)
      })
      .collect(),
      AppearancePage::TaskbarPosition => vec![
        format!(
          "{}{}",
          icon_label(argvus_tui::icons::INFO, tr(self.lang, "control_center.top")),
          if self.state.waybar_pos == TaskbarPosition::Top {
            format!(" · {}", tr(self.lang, "control_center.current"))
          } else {
            String::new()
          }
        ),
        format!(
          "{}{}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.bottom")
          ),
          if self.state.waybar_pos == TaskbarPosition::Bottom {
            format!(" · {}", tr(self.lang, "control_center.current"))
          } else {
            String::new()
          }
        ),
      ],
      AppearancePage::TaskbarSpaces => vec![
        format!(
          "{} · {}",
          icon_label(argvus_tui::icons::INFO, tr(self.lang, "control_center.top")),
          self.state.waybar_top
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.left")
          ),
          self.state.waybar_left
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.right")
          ),
          self.state.waybar_right
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.bottom")
          ),
          self.state.waybar_bottom
        ),
      ],
      AppearancePage::WidgetTelemetry => std::iter::once(format!(
        "[{}] {}",
        if self.state.widget_telemetry {
          "x"
        } else {
          " "
        },
        tr(self.lang, "control_center.enable")
      ))
      .chain(std::iter::once(
        tr(self.lang, "control_center.options").to_string(),
      ))
      .chain(WidgetTelemetryBlock::ALL.into_iter().map(|block| {
        format!(
          "[{}] {}",
          if self.state.widget_telemetry_blocks.enabled(block) {
            "x"
          } else {
            " "
          },
          tr(self.lang, block.label_key())
        )
      }))
      .collect(),
      AppearancePage::ControlPanel => self.control_panel_rows(),
      AppearancePage::WindowSpaces => vec![
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.inner_gap")
          ),
          self.state.gaps_in
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.outer_gap_top")
          ),
          self.state.gaps_out_top
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.outer_gap_left")
          ),
          self.state.gaps_out_left
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.outer_gap_right")
          ),
          self.state.gaps_out_right
        ),
        format!(
          "{} · {}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.outer_gap_bottom")
          ),
          self.state.gaps_out_bottom
        ),
      ],
      AppearancePage::GeneralBorders => vec![
        format!(
          "[{}] {} · {}",
          if self.state.rounded { "x" } else { " " },
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.rounded")
          ),
          if self.state.rounded {
            tr(self.lang, "control_center.enabled")
          } else {
            tr(self.lang, "control_center.disabled")
          }
        ),
        format!(
          "{} · {}{}",
          icon_label(
            argvus_tui::icons::INFO,
            tr(self.lang, "control_center.rounding")
          ),
          self.state.rounding,
          if self.state.rounded {
            String::new()
          } else {
            format!(" · {}", tr(self.lang, "control_center.disabled"))
          }
        ),
      ],
      AppearancePage::EdgeThickness => vec![format!(
        "{} · {}",
        icon_label(
          argvus_tui::icons::INFO,
          tr(self.lang, "control_center.thickness")
        ),
        self.state.thickness
      )],
      AppearancePage::AccentEdit | AppearancePage::Prompt { .. } => Vec::new(),
    }
  }

  /// Returns only cards whose required hardware is available on this host.
  fn control_panel_cards(&self) -> Vec<ControlPanelCard> {
    ControlPanelCard::ALL
      .into_iter()
      .filter(|card| self.state.control_panel_cards.available(*card))
      .collect()
  }

  /// Uses the uncommitted draft while editing, leaving persisted state intact
  /// until the user activates the Apply row.
  fn control_panel_view(&self) -> &ControlPanelCards {
    self
      .control_panel_draft
      .as_ref()
      .unwrap_or(&self.state.control_panel_cards)
  }

  fn control_panel_rows(&self) -> Vec<String> {
    self
      .control_panel_cards()
      .into_iter()
      .map(|card| {
        format!(
          "[{}] {}",
          if self.control_panel_view().enabled(card) {
            "x"
          } else {
            " "
          },
          tr(self.lang, card.label_key())
        )
      })
      .collect()
  }

  fn buttons(&self) -> Vec<Button> {
    if self.page != AppearancePage::ControlPanel {
      return Vec::new();
    }
    vec![Button::new(
      tr(self.lang, "control_center.apply_bc01e2"),
      ButtonKind::Primary,
    )]
  }

  fn toggle_buttons(&mut self, backwards: bool) {
    if self.on_buttons {
      self.on_buttons = false;
      if let Some(index) = self.button_from.take() {
        self.selected = index;
      }
    } else {
      self.button_from = Some(self.selected);
      self.button_selected = usize::from(backwards);
      self.on_buttons = true;
    }
  }

  fn move_button(&mut self, delta: isize) {
    let count = self.buttons().len();
    if count > 0 {
      self.button_selected =
        (self.button_selected as isize + delta).rem_euclid(count as isize) as usize;
    }
  }

  fn activate_button(&mut self) {
    if self.button_selected == 0 {
      self.apply_control_panel_changes();
    }
  }
  /// Executes the `hints` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn hints(&self) -> String {
    if self.page == AppearancePage::AccentEdit {
      format!(
        "{} · Enter {} · Esc {}",
        tr(self.lang, "control_center.hex_color"),
        tr(self.lang, "control_center.apply"),
        tr(self.lang, "control_center.back")
      )
    } else if matches!(
      self.page,
      AppearancePage::Themes | AppearancePage::ThemeFamilies { .. } | AppearancePage::CustomThemes
    ) {
      tr(self.lang, "control_center.theme_profile_themes_help").into()
    } else if self.page == AppearancePage::ThemeImport {
      tr(self.lang, "control_center.theme_profile_import_help").into()
    } else if self.page == AppearancePage::ThemeImportConfirm {
      tr(self.lang, "control_center.theme_profile_duplicate_help").into()
    } else if self.page == AppearancePage::ThemeDeleteConfirm {
      tr(self.lang, "control_center.theme_profile_delete_help").into()
    } else if matches!(self.page, AppearancePage::Prompt { .. }) {
      let (min, max) = if let AppearancePage::Prompt { goal } = self.page {
        goal.range()
      } else {
        (0, 100)
      };
      format!("0-9 edit · Enter confirm · Esc back · {min}..{max}")
    } else if self.page == AppearancePage::ControlPanel {
      tr(
        self.lang,
        "control_center.navigate_tab_actions_move_enter_activate_r_refresh_esc_back_help",
      )
      .into()
    } else if matches!(
      self.page,
      AppearancePage::Home | AppearancePage::SpacesBordersPosition
    ) {
      tr(
        self.lang,
        "control_center.jk_navigate_enter_open_space_toggle_r_refresh_esc_back",
      )
      .into()
    } else {
      tr(
        self.lang,
        "control_center.jk_navigate_enter_apply_r_refresh_esc_back",
      )
      .into()
    }
  }
  /// Renders `draw` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn draw(&mut self, frame: &mut Frame) {
    let area = shell(
      frame,
      frame.area(),
      &self.theme,
      &self.breadcrumb(),
      &self.hints(),
    );
    if self.page == AppearancePage::AccentEdit {
      self.draw_accent_editor(frame, area);
    } else if self.page == AppearancePage::Themes {
      self.draw_theme_page(frame, area);
    } else if self.page == AppearancePage::ThemeDeleteConfirm {
      let name = self
        .delete_theme
        .as_ref()
        .map(|theme| theme.name.as_str())
        .unwrap_or_default();
      frame.render_widget(
        Paragraph::new(vec![
          Line::from(tr(self.lang, "control_center.theme_profile_delete_title")),
          Line::from(name),
          Line::from(tr(
            self.lang,
            "control_center.theme_profile_delete_description",
          )),
          Line::from(Span::styled(
            format!(
              "{}  {}",
              if self.selected == 0 { ">" } else { " " },
              tr(self.lang, "control_center.theme_profile_delete")
            ),
            if self.selected == 0 {
              Style::new()
                .fg(self.theme.selected_foreground)
                .bg(self.theme.selected_background)
            } else {
              Style::new().fg(self.theme.foreground)
            },
          )),
          Line::from(Span::styled(
            format!(
              "{}  {}",
              if self.selected == 1 { ">" } else { " " },
              tr(self.lang, "control_center.cancel")
            ),
            if self.selected == 1 {
              Style::new()
                .fg(self.theme.selected_foreground)
                .bg(self.theme.selected_background)
            } else {
              Style::new().fg(self.theme.foreground)
            },
          )),
        ])
        .block(Block::default().borders(Borders::ALL)),
        area,
      );
    } else if self.page == AppearancePage::ThemeImportConfirm {
      let name = self.pending_import_name.as_deref().unwrap_or_default();
      frame.render_widget(
        Paragraph::new(vec![
          Line::from(tr(
            self.lang,
            "control_center.theme_profile_duplicate_title",
          )),
          Line::from(name),
          Line::from(tr(
            self.lang,
            "control_center.theme_profile_duplicate_description",
          )),
          Line::from(Span::styled(
            format!(
              "{}  {}",
              if self.selected == 0 { ">" } else { " " },
              tr(self.lang, "control_center.theme_profile_duplicate_replace")
            ),
            if self.selected == 0 {
              Style::new()
                .fg(self.theme.selected_foreground)
                .bg(self.theme.selected_background)
            } else {
              Style::new().fg(self.theme.foreground)
            },
          )),
          Line::from(Span::styled(
            format!(
              "{}  {}",
              if self.selected == 1 { ">" } else { " " },
              tr(self.lang, "control_center.cancel")
            ),
            if self.selected == 1 {
              Style::new()
                .fg(self.theme.selected_foreground)
                .bg(self.theme.selected_background)
            } else {
              Style::new().fg(self.theme.foreground)
            },
          )),
        ])
        .block(Block::default().borders(Borders::ALL)),
        area,
      );
    } else if let AppearancePage::Prompt { goal } = self.page {
      self.draw_prompt(frame, area, goal);
    } else {
      let rows = self.rows();
      let buttons = self.buttons();
      let (list_area, button_area) = if buttons.is_empty() {
        (area, None)
      } else {
        let button_height = argvus_tui::buttons::height(&buttons, area.width).min(area.height);
        let split =
          Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(area);
        (split[0], Some(split[1]))
      };
      list(
        frame,
        list_area,
        &self.theme,
        &rows,
        self.selected.min(rows.len().saturating_sub(1)),
      );
      if let Some(button_area) = button_area {
        let focus = if self.on_buttons {
          self.button_selected
        } else {
          usize::MAX
        };
        argvus_tui::buttons::draw(frame, button_area, &buttons, focus, &self.theme);
      }
    }
    if let Some(message) = &self.status {
      status(frame, area, &self.theme, message);
    }
    if self.status_loading {
      status(
        frame,
        area,
        &self.theme,
        &StatusMessage {
          kind: StatusKind::Info,
          text: tr(self.lang, "control_center.loading_appearance").into(),
        },
      );
    }
  }
  /// Renders `draw_prompt` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn draw_prompt(&mut self, frame: &mut Frame, area: Rect, goal: PromptGoal) {
    let label = tr(self.lang, self.prompt_label(goal));
    let chunks = Layout::vertical([
      Constraint::Length(3),
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Min(1),
    ])
    .split(area);
    frame.render_widget(
      Paragraph::new(Line::from(Span::styled(
        format!("{label}: {}", self.prompt_buffer),
        Style::new()
          .fg(self.theme.selected_foreground)
          .bg(self.theme.selected_background),
      ))),
      chunks[0],
    );
    let text = self.prompt_error.clone().unwrap_or_else(|| {
      if matches!(goal, PromptGoal::ExportProfile) {
        return format!(
          "{} · Enter confirm · Esc back",
          tr(self.lang, "control_center.theme_profile_export_name")
        );
      }
      let (min, max) = goal.range();
      format!("{min}..{max} · Enter confirm · Esc back")
    });
    frame.render_widget(
      Paragraph::new(Line::from(Span::styled(
        text,
        Style::new()
          .fg(if self.prompt_error.is_some() {
            self.theme.error
          } else {
            self.theme.foreground
          })
          .add_modifier(Modifier::DIM),
      ))),
      chunks[1],
    );
    if matches!(goal, PromptGoal::ExportProfile) {
      let output = backend::preview_theme_profile_path(&self.prompt_buffer)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "—".into());
      frame.render_widget(
        Paragraph::new(format!(
          "{}: {}",
          tr(self.lang, "control_center.theme_profile_output"),
          output
        ))
        .style(Style::new().fg(self.theme.muted)),
        chunks[2],
      );
    }
  }

  fn draw_accent_editor(&mut self, frame: &mut Frame, area: Rect) {
    let color = self.prompt_buffer.parse::<HexColor>().ok();
    let chunks = Layout::vertical([
      Constraint::Length(1),
      Constraint::Length(3),
      Constraint::Length(1),
      Constraint::Length(3),
      Constraint::Min(1),
    ])
    .split(area);
    frame.render_widget(
      Paragraph::new(tr(self.lang, "control_center.preview")),
      chunks[0],
    );
    let preview_style = color
      .map(|color| {
        Style::new()
          .bg(ratatui::style::Color::Rgb(color.r, color.g, color.b))
          .fg(color.foreground())
      })
      .unwrap_or_else(|| Style::new().fg(self.theme.error));
    frame.render_widget(
      Paragraph::new(Line::from(Span::styled(
        format!("  {}  ", self.prompt_buffer),
        preview_style,
      ))),
      chunks[1],
    );
    frame.render_widget(
      Paragraph::new(tr(self.lang, "control_center.hex_color")),
      chunks[2],
    );
    frame.render_widget(
      Paragraph::new(Line::from(Span::styled(
        format!("> {}", self.prompt_buffer),
        Style::new()
          .fg(self.theme.selected_foreground)
          .bg(self.theme.selected_background),
      ))),
      chunks[3],
    );
    if let Some(error) = &self.prompt_error {
      frame.render_widget(
        Paragraph::new(Span::styled(error, Style::new().fg(self.theme.error))),
        chunks[4],
      );
    }
  }

  fn draw_theme_page(&mut self, frame: &mut Frame, area: Rect) {
    let rows = self.rows();
    list(
      frame,
      area,
      &self.theme,
      &rows,
      self.selected.min(rows.len().saturating_sub(1)),
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  /// Executes the `app` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn app(page: AppearancePage) -> AppearanceApp {
    AppearanceApp {
      page,
      lang: Lang::for_locale("en-US"),
      theme: Theme::load(),
      status: None,
      state: AppearanceState::default(),
      loaded: true,
      status_loading: false,
      selected: 0,
      prompt_buffer: String::new(),
      prompt_error: None,
      prompt_back: None,
      job: None,
      refreshed: Vec::new(),
      action: None,
      reload_requested: false,
      control_panel_draft: None,
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      delete_theme: None,
      import_archives: Vec::new(),
      pending_import: None,
      pending_import_name: None,
      manager: JobManager::default(),
    }
  }
  #[test]
  /// Executes the `home_has_categories_and_spacing_is_nested` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn home_has_categories_and_spacing_is_nested() {
    assert_eq!(app(AppearancePage::Home).rows().len(), 8);
    assert_eq!(app(AppearancePage::SpacesBordersPosition).rows().len(), 5);
    assert_eq!(app(AppearancePage::Taskbar).rows().len(), 2);
    assert_eq!(app(AppearancePage::WidgetTelemetry).rows().len(), 9);
    assert_eq!(app(AppearancePage::ControlPanel).rows().len(), 14);
    assert_eq!(app(AppearancePage::Effects).rows().len(), 2);
  }
  #[test]
  fn theme_rows_use_friendly_official_names() {
    let rows = app(AppearancePage::Themes).rows();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], "Dark >");
    assert_eq!(rows[1], "Light >");
    assert!(rows[2].contains("Custom"));
  }
  #[test]
  fn theme_families_are_grouped_by_identifier_category() {
    let dark_rows = app(AppearancePage::ThemeFamilies {
      category: ThemeCategory::Dark,
    })
    .rows();
    let light_rows = app(AppearancePage::ThemeFamilies {
      category: ThemeCategory::Light,
    })
    .rows();
    assert_eq!(dark_rows.len(), 11);
    assert_eq!(light_rows.len(), 5);
    assert!(dark_rows.iter().any(|row| row.contains("ARGVUS Gruvbox >")));
    assert!(light_rows.iter().any(|row| row.contains("ARGVUS GitHub >")));
    assert!(!dark_rows.iter().any(|row| row.contains("Dark Gruvbox")));
  }
  #[test]
  /// Executes the `home_rows_follow_the_global_icon_setting` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn home_rows_follow_the_global_icon_setting() {
    AppConfig::set_session_icons(true);
    let with_icons = app(AppearancePage::Home).home_rows();
    assert!(with_icons[0].starts_with(&format!("{} Theme", argvus_tui::icons::PALETTE)));
    assert!(with_icons[3].starts_with(&format!("{} Spaces", argvus_tui::icons::STORAGE)));

    AppConfig::set_session_icons(false);
    let without_icons = app(AppearancePage::Home).home_rows();
    assert!(without_icons[0].starts_with("Theme"));
    assert!(without_icons[3].starts_with("Spaces"));
    assert!(!without_icons.iter().any(|row| row.starts_with(' ')));
    AppConfig::set_session_icons(true);
  }
  #[test]
  /// Executes the `selection_bounds_follow_each_page` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn selection_bounds_follow_each_page() {
    let mut a = app(AppearancePage::WindowSpaces);
    a.handle(KeyCode::End);
    assert_eq!(a.selected, 4);
    a.handle(KeyCode::Down);
    assert_eq!(a.selected, 4);
  }
  #[test]
  /// Executes the `disabled_rounding_does_not_open_prompt` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn disabled_rounding_does_not_open_prompt() {
    let mut a = app(AppearancePage::GeneralBorders);
    a.selected = 1;
    a.handle(KeyCode::Enter);
    assert!(a.prompt_back.is_none());
  }

  #[test]
  fn control_panel_apply_is_reached_through_tab_actions() {
    let mut a = app(AppearancePage::ControlPanel);
    assert!(!a.rows().iter().any(|row| row.contains("Apply")));
    a.handle(KeyCode::Tab);
    assert!(a.on_buttons);
    assert_eq!(a.button_selected, 0);
    a.handle(KeyCode::BackTab);
    assert!(!a.on_buttons);
  }
}
