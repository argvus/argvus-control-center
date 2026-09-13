use crate::{
  backend,
  model::{
    ACCENTS, AppearancePage, AppearanceState, PromptGoal, THEMES, accent_label, theme_label,
  },
};
use argvus_control_center_core::jobs::{JobHandle, JobManager, JobState};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::components::{StatusKind, StatusMessage};
use argvus_tui::page::{list, shell, status};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Clone)]
enum JobData {
  Loaded(AppearanceState),
  Action(String),
}

/// Keyboard-first appearance settings: theme, accent, wallpaper, taskbar
/// position, spacing gaps, interface effects and widget telemetry. Every
/// action farms out to the shared ARGVUS shell scripts so this TUI stays in
/// sync with the graphical control panel.
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
      text: tr(
        self.lang,
        "Carregando aparência...",
        "Loading appearance...",
      )
      .into(),
    });
  }

  fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    self.job = Some(
      self
        .manager
        .spawn(|_| Ok(JobData::Loaded(backend::load_state()))),
    );
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
          if self.status_loading {
            self.status = None;
          }
          self.status_loading = false;
        }
        Ok(JobData::Action(_)) => {}
        Err(error) => {
          self.status_loading = false;
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: format!("{} {error}", tr(self.lang, "Erro:", "Error:")),
          });
        }
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
          });
        }
        Ok(_) => {}
        Err(error) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: format!("{} {error}", tr(self.lang, "Erro:", "Error:")),
          });
        }
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

  fn toggle(&mut self, effects: bool) {
    if effects {
      let enable = !self.state.effects;
      self.apply(
        tr(self.lang, "Efeitos aplicados", "Effects applied").into(),
        move || backend::set_effects(enable),
      );
    } else {
      let enable = !self.state.widget_telemetry;
      self.apply(
        tr(
          self.lang,
          "Telemetria do widget alterada",
          "Widget telemetry changed",
        )
        .into(),
        move || backend::set_telemetry(enable),
      );
    }
  }

  fn pick(&mut self) {
    match self.page {
      AppearancePage::Themes => {
        if let Some((name, _)) = THEMES.get(self.selected) {
          let name = name.to_string();
          self.apply(
            tr(self.lang, "Tema aplicado", "Theme applied").into(),
            move || backend::set_theme(&name),
          );
        }
      }
      AppearancePage::Accents => {
        if let Some((_, color)) = ACCENTS.get(self.selected) {
          let color = color.to_string();
          self.apply(
            tr(self.lang, "Cor de destaque aplicada", "Accent applied").into(),
            move || backend::set_accent(&color),
          );
        }
      }
      AppearancePage::Wallpapers => {
        if self.selected == 0 {
          self.apply(
            tr(self.lang, "Seletor de papel de parede aberto", "Wallpaper chooser opened").into(),
            backend::choose_wallpaper,
          );
        } else if let Some(name) = self.state.wallpapers.get(self.selected - 1) {
          let name = name.clone();
          self.apply(
            tr(self.lang, "Papel de parede aplicado", "Wallpaper applied").into(),
            move || backend::set_wallpaper(&name),
          );
        }
      }
      AppearancePage::WaybarPosition => {
        let position = if self.selected.is_multiple_of(2) {
          "top"
        } else {
          "bottom"
        };
        self.apply(
          tr(
            self.lang,
            "Posição da barra alterada",
            "Taskbar position changed",
          )
          .into(),
          move || backend::set_waybar_position(position),
        );
      }
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
      KeyCode::Enter => {
        let goal = match self.page {
          AppearancePage::Prompt { goal } => goal,
          _ => PromptGoal::GapsIn,
        };
        let value = self.prompt_buffer.trim().to_string();
        let parsed = value.parse::<i32>().ok();
        if !matches!(parsed, Some(0..=100)) {
          self.prompt_error = Some(
            tr(
              self.lang,
              "Digite um número inteiro válido",
              "Enter a valid integer",
            )
            .into(),
          );
          return false;
        }
        let back = self.prompt_back.take();
        let key = goal.key();
        self.apply(
          tr(self.lang, "Espaçamento aplicado", "Spacing applied").into(),
          move || backend::set_spaces(key, &value),
        );
        self.page = back.unwrap_or(AppearancePage::Home);
        self.prompt_buffer.clear();
        self.prompt_error = None;
        false
      }
      KeyCode::Esc | KeyCode::Left => {
        let back = self.prompt_back.take();
        self.page = back.unwrap_or(AppearancePage::Home);
        self.prompt_buffer.clear();
        self.prompt_error = None;
        false
      }
      KeyCode::Char(character) if character.is_ascii_digit() => {
        if self.prompt_buffer.len() < 3 {
          self.prompt_buffer.push(character);
        }
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
      match self.page {
        AppearancePage::Home => return true,
        AppearancePage::Themes
        | AppearancePage::Wallpapers
        | AppearancePage::Accents
        | AppearancePage::WaybarPosition => {
          self.page = AppearancePage::Home;
          self.selected = 0;
        }
        AppearancePage::Prompt { .. } => {}
      }
      return false;
    }
    match self.page {
      AppearancePage::Home => match key {
        KeyCode::Char('r') => self.refresh(),
        KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => self.selected = self.selected.saturating_add(1),
        KeyCode::Home => self.selected = 0,
        KeyCode::End => self.selected = Self::selection_len_home().saturating_sub(1),
        KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => self.open_home_choice(),
        _ => {}
      },
      AppearancePage::Themes
      | AppearancePage::Wallpapers
      | AppearancePage::Accents
      | AppearancePage::WaybarPosition => match key {
        KeyCode::Char('r') => self.refresh(),
        KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => self.selected = self.selected.saturating_add(1),
        KeyCode::Home => self.selected = 0,
        KeyCode::End => self.selected = self.picker_len().saturating_sub(1),
        KeyCode::Enter | KeyCode::Right => self.pick(),
        _ => {}
      },
      AppearancePage::Prompt { .. } => {}
    }
    false
  }

  fn open_home_choice(&mut self) {
    match self.selected {
      0 => self.page = AppearancePage::Themes,
      1 => self.page = AppearancePage::Accents,
      2 => self.page = AppearancePage::Wallpapers,
      3 => self.page = AppearancePage::WaybarPosition,
      4 => self.open_prompt(PromptGoal::GapsIn),
      5 => self.open_prompt(PromptGoal::GapsOut),
      6 => self.open_prompt(PromptGoal::Waybar),
      7 => self.toggle(true),
      8 => self.toggle(false),
      _ => {}
    }
    self.selected = 0;
  }

  const fn selection_len_home() -> usize {
    9
  }

  fn picker_len(&self) -> usize {
    match self.page {
      AppearancePage::Themes => THEMES.len(),
      AppearancePage::Accents => ACCENTS.len(),
      AppearancePage::Wallpapers => self.state.wallpapers.len() + 1,
      AppearancePage::WaybarPosition => 2,
      _ => 0,
    }
  }

  fn home_rows(&self) -> Vec<String> {
    let enabled = tr(self.lang, "Ativado", "Enabled");
    let disabled = tr(self.lang, "Desativado", "Disabled");
    let position_value = if self.state.waybar_pos == "bottom" {
      tr(self.lang, "Inferior", "Bottom")
    } else {
      tr(self.lang, "Superior", "Top")
    };
    vec![
      format!(
        "{} · {}",
        tr(self.lang, "Tema", "Theme"),
        theme_label(&self.state.theme)
      ),
      format!(
        "{} · {}",
        tr(self.lang, "Cor de destaque", "Highlight color"),
        accent_label(&self.state.accent)
      ),
      format!(
        "{} · {}",
        tr(self.lang, "Papel de parede", "Wallpaper"),
        self
          .state
          .wallpaper_active
          .clone()
          .unwrap_or_else(|| tr(self.lang, "nenhum", "none").to_string())
      ),
      format!(
        "{} · {}",
        tr(self.lang, "Posição da barra de tarefas", "Taskbar position"),
        position_value
      ),
      format!(
        "{} · {}",
        tr(
          self.lang,
          "Janelas: espaçamento interno",
          "Windows: inner gap"
        ),
        self.state.gaps_in
      ),
      format!(
        "{} · {}",
        tr(
          self.lang,
          "Janelas: espaçamento externo",
          "Windows: outer gap"
        ),
        self.state.gaps_out
      ),
      format!(
        "{} · {}",
        tr(self.lang, "Barra de tarefas: margem", "Taskbar: margin"),
        self.state.waybar
      ),
      format!(
        "[{}] {} · {}",
        if self.state.effects { "x" } else { " " },
        tr(self.lang, "Efeitos de interface", "Interface effects"),
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
        tr(self.lang, "Telemetria do widget", "Widget telemetry"),
        if self.state.widget_telemetry {
          enabled
        } else {
          disabled
        }
      ),
    ]
  }

  fn breadcrumb(&self) -> String {
    let title = tr(self.lang, "Aparência", "Appearance").to_string();
    let page = match self.page {
      AppearancePage::Home => tr(self.lang, "Aparência", "Appearance").to_string(),
      AppearancePage::Themes => tr(self.lang, "Temas", "Themes").to_string(),
      AppearancePage::Wallpapers => tr(self.lang, "Papéis de parede", "Wallpapers").to_string(),
      AppearancePage::Accents => tr(self.lang, "Cor de destaque", "Highlight color").to_string(),
      AppearancePage::WaybarPosition => {
        tr(self.lang, "Posição da barra", "Taskbar position").to_string()
      }
      AppearancePage::Prompt { goal } => match goal {
        PromptGoal::GapsIn => tr(self.lang, "Espaçamento interno", "Inner gap").to_string(),
        PromptGoal::GapsOut => tr(self.lang, "Espaçamento externo", "Outer gap").to_string(),
        PromptGoal::Waybar => tr(self.lang, "Margem da barra", "Taskbar margin").to_string(),
      },
    };
    format!("{title} › {page}")
  }

  fn hints(&self) -> String {
    match self.page {
      AppearancePage::Prompt { .. } => tr(
        self.lang,
        "0-9 editar · Enter confirmar · Esc voltar",
        "0-9 edit · Enter confirm · Esc back",
      ),
      AppearancePage::Home => tr(
        self.lang,
        "↑↓/jk navegar · Enter abrir · Espaço alternar · r atualizar · Esc voltar",
        "↑↓/jk navigate · Enter open · Space toggle · r refresh · Esc back",
      ),
      _ => tr(
        self.lang,
        "↑↓/jk navegar · Enter aplicar · r atualizar · Esc voltar",
        "↑↓/jk navigate · Enter apply · r refresh · Esc back",
      ),
    }
    .to_string()
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = shell(
      frame,
      frame.area(),
      &self.theme,
      &self.breadcrumb(),
      &self.hints(),
    );
    match self.page {
      AppearancePage::Home => {
        let rows = self.home_rows();
        list(frame, area, &self.theme, &rows, self.selected);
      }
      AppearancePage::Themes => {
        let rows = THEMES
          .iter()
          .map(|(name, label)| {
            if self.state.theme == *name {
              format!("{label} · {}", tr(self.lang, "atual", "current"))
            } else {
              label.to_string()
            }
          })
          .collect::<Vec<_>>();
        list(frame, area, &self.theme, &rows, self.selected);
      }
      AppearancePage::Accents => {
        let rows = ACCENTS
          .iter()
          .map(|(label, color)| {
            if self.state.accent == *color {
              format!("{label} ({color}) · {}", tr(self.lang, "atual", "current"))
            } else {
              format!("{label} ({color})")
            }
          })
          .collect::<Vec<_>>();
        list(frame, area, &self.theme, &rows, self.selected);
      }
      AppearancePage::Wallpapers => {
        let mut rows = vec![tr(
          self.lang,
          "Escolher imagem da HOME…",
          "Choose image from HOME…",
        ).to_string()];
        rows.extend(self
          .state
          .wallpapers
          .iter()
          .map(|name| {
            if self.state.wallpaper_active.as_deref() == Some(name.as_str()) {
              format!("{name} · {}", tr(self.lang, "atual", "current"))
            } else {
              name.clone()
            }
          })
          .collect::<Vec<_>>());
        if rows.is_empty() {
          self.draw_empty(
            frame,
            area,
            tr(
              self.lang,
              "Nenhum papel de parede em /usr/share/backgrounds/argvus",
              "No wallpapers in /usr/share/backgrounds/argvus",
            )
            .to_string(),
          );
        } else {
          list(frame, area, &self.theme, &rows, self.selected);
        }
      }
      AppearancePage::WaybarPosition => {
        let rows = [
          format!(
            "{} {}",
            tr(self.lang, "Superior", "Top"),
            if self.state.waybar_pos == "top" {
              "· atual"
            } else {
              ""
            }
          ),
          format!(
            "{} {}",
            tr(self.lang, "Inferior", "Bottom"),
            if self.state.waybar_pos == "bottom" {
              "· atual"
            } else {
              ""
            }
          ),
        ];
        list(frame, area, &self.theme, &rows, self.selected);
      }
      AppearancePage::Prompt { goal } => {
        self.draw_prompt(frame, area, goal);
      }
    }
    if let Some(message) = &self.status {
      status(frame, area, &self.theme, message);
    }
  }

  fn draw_empty(&self, frame: &mut Frame, area: Rect, text: String) {
    frame.render_widget(
      Paragraph::new(Line::from(Span::styled(
        text,
        Style::new()
          .fg(self.theme.foreground)
          .add_modifier(Modifier::DIM),
      ))),
      area.inner(Margin::new(1, 0)),
    );
  }

  fn draw_prompt(&mut self, frame: &mut Frame, area: Rect, goal: PromptGoal) {
    let label = match goal {
      PromptGoal::GapsIn => tr(
        self.lang,
        "Janelas: espaçamento interno",
        "Windows: inner gap",
      ),
      PromptGoal::GapsOut => tr(
        self.lang,
        "Janelas: espaçamento externo",
        "Windows: outer gap",
      ),
      PromptGoal::Waybar => tr(self.lang, "Barra de tarefas: margem", "Taskbar: margin"),
    };
    let chunks = Layout::vertical([
      Constraint::Length(3),
      Constraint::Length(1),
      Constraint::Min(1),
    ])
    .split(area);
    let input = Paragraph::new(Line::from(Span::styled(
      format!("{label}: {}", self.prompt_buffer),
      Style::new()
        .fg(self.theme.selected_foreground)
        .bg(self.theme.selected_background),
    )));
    frame.render_widget(input, chunks[0]);
    if let Some(error) = &self.prompt_error {
      frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
          error.clone(),
          Style::new().fg(self.theme.error),
        ))),
        chunks[1],
      );
    }
    if self.prompt_error.is_none() {
      let hint = Line::from(Span::styled(
        tr(
          self.lang,
          "0 a 100 · independente do tema",
          "0 to 100 · independent of the theme",
        ),
        Style::new()
          .fg(self.theme.foreground)
          .add_modifier(Modifier::DIM),
      ));
      frame.render_widget(Paragraph::new(hint), chunks[1]);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::AppearancePage;

  #[test]
  fn home_has_nine_rows_leading_to_pages() {
    let app = AppearanceApp {
      page: AppearancePage::Home,
      lang: Lang::Pt,
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
    };
    assert_eq!(app.home_rows().len(), 9);
  }

  #[test]
  fn waybar_position_picker_length() {
    let mut app = AppearanceApp {
      page: AppearancePage::WaybarPosition,
      lang: Lang::Pt,
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
    };
    assert_eq!(app.picker_len(), 2);
    app.selected = 1;
    assert!(!app.handle(KeyCode::Enter));
  }
}
