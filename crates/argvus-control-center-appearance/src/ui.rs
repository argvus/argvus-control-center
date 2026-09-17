use crate::{
  backend,
  model::{
    ACCENTS, AppearancePage, AppearanceState, PromptGoal, THEME_FAMILIES, TaskbarPosition,
    accent_label, theme_family_label,
  },
};
use argvus_control_center_core::{
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
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
};

#[derive(Debug, Clone)]
enum JobData {
  Loaded(AppearanceState),
  Action(String),
}

fn icon_label(icon: &'static str, label: impl AsRef<str>) -> String {
  let label = label.as_ref();
  let icon = AppConfig::icon(icon);
  if icon.is_empty() {
    label.to_string()
  } else {
    format!("{icon} {label}")
  }
}

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
  action: Option<JobHandle<JobData>>,
  reload_requested: bool,
  manager: JobManager,
}
impl AppearanceApp {
  pub fn reload(&mut self) {
    self.refresh();
  }
  pub fn new(lang: Lang, theme: Theme) -> Self {
    let mut app = Self {
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
      action: None,
      reload_requested: false,
      manager: JobManager::default(),
    };
    app.refresh();
    app.trigger_first_load();
    app
  }
  fn trigger_first_load(&mut self) {
    self.status_loading = true;
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "control_center.loading_appearance").into(),
    });
  }
  fn refresh(&mut self) {
    if self.job.is_none() {
      self.job = Some(
        self
          .manager
          .spawn(|_| Ok(JobData::Loaded(backend::load_state()))),
      );
    }
  }
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(JobData::Loaded(state)) => {
          self.state = state;
          self.loaded = true;
          let was_loading = self.status_loading;
          self.status_loading = false;
          if was_loading {
            self.status = None;
          }
        }
        Err(error) => {
          self.status_loading = false;
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: format!("{} {error}", tr(self.lang, "control_center.error")),
          });
        }
        Ok(JobData::Action(_)) => {}
      }
      changed = true;
    }
    if let Some(action) = &self.action
      && let JobState::Finished(result) = action.try_state()
    {
      self.action = None;
      if self.reload_requested {
        self.reload_requested = false;
        self.refresh();
      }
      match result {
        Ok(JobData::Action(text)) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: text.clone(),
          })
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
  fn go(&mut self, page: AppearancePage) {
    self.page = page;
    self.selected = 0;
  }
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
  fn pick(&mut self) {
    match self.page {
      AppearancePage::Themes if self.selected < THEME_FAMILIES.len() => {
        self.go(AppearancePage::ThemeModes {
          family: self.selected,
        })
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
        if let Some((_, color)) = ACCENTS.get(self.selected) {
          let color = color.to_string();
          self.apply(
            tr(self.lang, "control_center.accent_applied").into(),
            move || backend::set_accent(&color),
          );
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
  fn open_prompt(&mut self, goal: PromptGoal) {
    self.prompt_back = Some(self.page);
    self.page = AppearancePage::Prompt { goal };
    self.prompt_buffer.clear();
    self.prompt_error = None;
  }
  fn prompt_key(&mut self, key: KeyCode) -> bool {
    match key {
      KeyCode::Enter if self.action.is_none() => {
        let AppearancePage::Prompt { goal } = self.page else {
          return false;
        };
        let value = self.prompt_buffer.trim().to_string();
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
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.prompt_back.is_some() {
      return self.prompt_key(key);
    }
    if self.job.is_some() || self.action.is_some() {
      return false;
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
  fn back(&mut self) -> bool {
    if self.page == AppearancePage::Home {
      return true;
    }
    match self.page {
      AppearancePage::Home => unreachable!(),
      AppearancePage::Themes
      | AppearancePage::Wallpapers
      | AppearancePage::Accents
      | AppearancePage::SpacesBordersPosition => {
        self.go(AppearancePage::Home);
      }
      AppearancePage::ThemeModes { .. } => {
        self.go(AppearancePage::Themes);
      }
      AppearancePage::TaskbarPosition
      | AppearancePage::TaskbarSpaces
      | AppearancePage::WindowSpaces
      | AppearancePage::GeneralBorders
      | AppearancePage::EdgeThickness => {
        self.go(AppearancePage::SpacesBordersPosition);
      }
      AppearancePage::Prompt { .. } => {}
    }
    false
  }
  fn open_or_pick(&mut self) {
    match self.page {
      AppearancePage::Home => match self.selected {
        0 => self.go(AppearancePage::Themes),
        1 => self.go(AppearancePage::Accents),
        2 => self.go(AppearancePage::Wallpapers),
        3 => self.go(AppearancePage::SpacesBordersPosition),
        4 => self.apply_toggle_effects(),
        5 => self.apply_toggle_telemetry(),
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
      | AppearancePage::ThemeModes { .. }
      | AppearancePage::Wallpapers
      | AppearancePage::Accents
      | AppearancePage::TaskbarPosition
      | AppearancePage::TaskbarSpaces
      | AppearancePage::WindowSpaces
      | AppearancePage::GeneralBorders
      | AppearancePage::EdgeThickness => self.pick(),
      AppearancePage::Prompt { .. } => {}
    }
  }
  fn apply_toggle_effects(&mut self) {
    let value = !self.state.effects;
    self.apply(
      tr(self.lang, "control_center.effects_applied").into(),
      move || backend::set_effects(value),
    );
  }
  fn apply_toggle_telemetry(&mut self) {
    let value = !self.state.widget_telemetry;
    self.apply(
      tr(self.lang, "control_center.widget_telemetry_changed").into(),
      move || backend::set_telemetry(value),
    );
  }
  fn selection_len(&self) -> usize {
    match self.page {
      AppearancePage::Home => 6,
      AppearancePage::Themes => THEME_FAMILIES.len(),
      AppearancePage::Accents => ACCENTS.len(),
      AppearancePage::ThemeModes { .. } | AppearancePage::TaskbarPosition => 2,
      AppearancePage::Wallpapers => self.state.wallpapers.len() + 1,
      AppearancePage::SpacesBordersPosition => 5,
      AppearancePage::TaskbarSpaces => 4,
      AppearancePage::WindowSpaces => 5,
      AppearancePage::GeneralBorders => 2,
      AppearancePage::EdgeThickness => 1,
      AppearancePage::Prompt { .. } => 0,
    }
  }
  fn home_rows(&self) -> Vec<String> {
    let enabled = tr(self.lang, "control_center.enabled");
    let disabled = tr(self.lang, "control_center.disabled");
    vec![
      format!(
        "{} · {} [{}]",
        icon_label("🎨", tr(self.lang, "control_center.theme")),
        theme_family_label(&self.state.theme),
        if self.state.is_float_theme() {
          tr(self.lang, "control_center.theme_mode_float")
        } else {
          tr(self.lang, "control_center.theme_mode_sticky")
        }
      ),
      format!(
        "{} · {}",
        icon_label("🌈", tr(self.lang, "control_center.highlight_color")),
        accent_label(&self.state.accent)
      ),
      format!(
        "{} · {}",
        icon_label("🖼️", tr(self.lang, "control_center.wallpaper")),
        self
          .state
          .wallpaper_active
          .clone()
          .unwrap_or_else(|| tr(self.lang, "control_center.none").to_string())
      ),
      icon_label(
        "📐",
        tr(self.lang, "control_center.spaces_borders_position"),
      ),
      format!(
        "[{}] {} · {}",
        if self.state.effects { "x" } else { " " },
        icon_label("✨", tr(self.lang, "control_center.interface_effects")),
        if self.state.effects {
          enabled
        } else {
          disabled
        }
      ),
      format!(
        "[{}] {} · {}",
        if self.state.widget_telemetry {
          "x"
        } else {
          " "
        },
        icon_label("📊", tr(self.lang, "control_center.widget_telemetry")),
        if self.state.widget_telemetry {
          enabled
        } else {
          disabled
        }
      ),
    ]
  }
  fn prompt_label(&self, goal: PromptGoal) -> &'static str {
    match goal {
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
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "control_center.appearance");
    match self.page {
      AppearancePage::Home => root.into(),
      AppearancePage::Themes => format!("{root} › {}", tr(self.lang, "control_center.themes")),
      AppearancePage::ThemeModes { .. } => {
        format!("{root} › {}", tr(self.lang, "control_center.themes"))
      }
      AppearancePage::Wallpapers => {
        format!("{root} › {}", tr(self.lang, "control_center.wallpapers"))
      }
      AppearancePage::Accents => format!(
        "{root} › {}",
        tr(self.lang, "control_center.highlight_color")
      ),
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
  fn rows(&self) -> Vec<String> {
    match self.page {
      AppearancePage::Home => self.home_rows(),
      AppearancePage::Themes => THEME_FAMILIES
        .iter()
        .map(|(name, label)| {
          let current = self
            .state
            .theme
            .strip_suffix("-float")
            .unwrap_or(&self.state.theme);
          let suffix = if current == *name {
            format!(" · {}", tr(self.lang, "control_center.current"))
          } else {
            String::new()
          };
          format!("{label}{suffix} >")
        })
        .collect(),
      AppearancePage::ThemeModes { .. } => [
        (
          "📌",
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
      AppearancePage::Accents => ACCENTS
        .iter()
        .map(|(label, color)| {
          let suffix = if self.state.accent == *color {
            format!(" · {}", tr(self.lang, "control_center.current"))
          } else {
            String::new()
          };
          format!("{label} ({color}){suffix}")
        })
        .collect(),
      AppearancePage::SpacesBordersPosition => vec![
        icon_label("↕", tr(self.lang, "control_center.taskbar_position")),
        icon_label("📏", tr(self.lang, "control_center.taskbar_spaces")),
        icon_label("📐", tr(self.lang, "control_center.window_spaces")),
        icon_label("◯", tr(self.lang, "control_center.general_borders")),
        icon_label("▰", tr(self.lang, "control_center.edge_thickness")),
      ],
      AppearancePage::TaskbarPosition => vec![
        format!(
          "{}{}",
          icon_label("↥", tr(self.lang, "control_center.top")),
          if self.state.waybar_pos == TaskbarPosition::Top {
            format!(" · {}", tr(self.lang, "control_center.current"))
          } else {
            String::new()
          }
        ),
        format!(
          "{}{}",
          icon_label("↧", tr(self.lang, "control_center.bottom")),
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
          icon_label("↥", tr(self.lang, "control_center.top")),
          self.state.waybar_top
        ),
        format!(
          "{} · {}",
          icon_label("↤", tr(self.lang, "control_center.left")),
          self.state.waybar_left
        ),
        format!(
          "{} · {}",
          icon_label("↦", tr(self.lang, "control_center.right")),
          self.state.waybar_right
        ),
        format!(
          "{} · {}",
          icon_label("↧", tr(self.lang, "control_center.bottom")),
          self.state.waybar_bottom
        ),
      ],
      AppearancePage::WindowSpaces => vec![
        format!(
          "{} · {}",
          icon_label("↔", tr(self.lang, "control_center.inner_gap")),
          self.state.gaps_in
        ),
        format!(
          "{} · {}",
          icon_label("↥", tr(self.lang, "control_center.outer_gap_top")),
          self.state.gaps_out_top
        ),
        format!(
          "{} · {}",
          icon_label("↤", tr(self.lang, "control_center.outer_gap_left")),
          self.state.gaps_out_left
        ),
        format!(
          "{} · {}",
          icon_label("↦", tr(self.lang, "control_center.outer_gap_right")),
          self.state.gaps_out_right
        ),
        format!(
          "{} · {}",
          icon_label("↧", tr(self.lang, "control_center.outer_gap_bottom")),
          self.state.gaps_out_bottom
        ),
      ],
      AppearancePage::GeneralBorders => vec![
        format!(
          "[{}] {} · {}",
          if self.state.rounded { "x" } else { " " },
          icon_label("◯", tr(self.lang, "control_center.rounded")),
          if self.state.rounded {
            tr(self.lang, "control_center.enabled")
          } else {
            tr(self.lang, "control_center.disabled")
          }
        ),
        format!(
          "{} · {}{}",
          icon_label("⌒", tr(self.lang, "control_center.rounding")),
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
        icon_label("▰", tr(self.lang, "control_center.thickness")),
        self.state.thickness
      )],
      AppearancePage::Prompt { .. } => Vec::new(),
    }
  }
  fn hints(&self) -> String {
    if matches!(self.page, AppearancePage::Prompt { .. }) {
      let (min, max) = if let AppearancePage::Prompt { goal } = self.page {
        goal.range()
      } else {
        (0, 100)
      };
      format!("0-9 edit · Enter confirm · Esc back · {min}..{max}")
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
  pub fn draw(&mut self, frame: &mut Frame) {
    let area = shell(
      frame,
      frame.area(),
      &self.theme,
      &self.breadcrumb(),
      &self.hints(),
    );
    if let AppearancePage::Prompt { goal } = self.page {
      self.draw_prompt(frame, area, goal);
    } else {
      let rows = self.rows();
      list(
        frame,
        area,
        &self.theme,
        &rows,
        self.selected.min(rows.len().saturating_sub(1)),
      );
    }
    if let Some(message) = &self.status {
      status(frame, area, &self.theme, message);
    }
  }
  fn draw_prompt(&mut self, frame: &mut Frame, area: Rect, goal: PromptGoal) {
    let label = tr(self.lang, self.prompt_label(goal));
    let chunks = Layout::vertical([
      Constraint::Length(3),
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
  }
}

#[cfg(test)]
mod tests {
  use super::*;
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
      action: None,
      reload_requested: false,
      manager: JobManager::default(),
    }
  }
  #[test]
  fn home_has_categories_and_spacing_is_nested() {
    assert_eq!(app(AppearancePage::Home).rows().len(), 6);
    assert_eq!(app(AppearancePage::SpacesBordersPosition).rows().len(), 5);
  }
  #[test]
  fn home_rows_follow_the_global_icon_setting() {
    AppConfig::set_session_icons(true);
    let with_icons = app(AppearancePage::Home).home_rows();
    assert!(with_icons[0].starts_with("🎨 Theme"));
    assert!(with_icons[3].starts_with("📐 Spaces"));

    AppConfig::set_session_icons(false);
    let without_icons = app(AppearancePage::Home).home_rows();
    assert!(without_icons[0].starts_with("Theme"));
    assert!(without_icons[3].starts_with("Spaces"));
    assert!(!without_icons.iter().any(|row| row.starts_with(' ')));
    AppConfig::set_session_icons(true);
  }
  #[test]
  fn selection_bounds_follow_each_page() {
    let mut a = app(AppearancePage::WindowSpaces);
    a.handle(KeyCode::End);
    assert_eq!(a.selected, 4);
    a.handle(KeyCode::Down);
    assert_eq!(a.selected, 4);
  }
  #[test]
  fn disabled_rounding_does_not_open_prompt() {
    let mut a = app(AppearancePage::GeneralBorders);
    a.selected = 1;
    a.handle(KeyCode::Enter);
    assert!(a.prompt_back.is_none());
  }
}
