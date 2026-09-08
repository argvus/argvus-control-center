use argvus_control_center_about::{App as AboutState, Tab};
use argvus_control_center_settings::{App as SettingsState, Page};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
  Home,
  Settings,
  About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialRoute {
  Home,
  Apps,
  Fonts,
  LocaleRegion,
  Language,
  System,
  About(Tab),
}

pub struct App {
  pub lang: Lang,
  pub theme: Theme,
  pub route: Route,
  pub home_selected: usize,
  pub settings: SettingsState,
  pub about: AboutState,
  pub help: bool,
  pub quit: bool,
  pub width: u16,
  pub height: u16,
}

impl App {
  pub fn new(initial: InitialRoute) -> Self {
    let lang = Lang::detect();
    let theme = Theme::load();
    let (route, page, tab) = match initial {
      InitialRoute::Home => (Route::Home, Page::Main, Tab::System),
      InitialRoute::Apps => (Route::Settings, Page::DefaultApps, Tab::System),
      InitialRoute::Fonts => (Route::Settings, Page::Fonts, Tab::System),
      InitialRoute::LocaleRegion => (Route::Settings, Page::LocaleRegion, Tab::System),
      InitialRoute::Language => (Route::Settings, Page::Language, Tab::System),
      InitialRoute::System => (Route::Settings, Page::System, Tab::System),
      InitialRoute::About(tab) => (Route::About, Page::Main, tab),
    };
    Self {
      lang,
      theme: theme.clone(),
      route,
      home_selected: 0,
      settings: SettingsState::with_context(page, lang, theme.clone()),
      about: AboutState::with_context(tab, lang, theme),
      help: false,
      quit: false,
      width: 80,
      height: 24,
    }
  }

  pub fn home_rows(&self) -> [&'static str; 6] {
    [
      tr(self.lang, "Apps Padrão", "Default Apps"),
      tr(self.lang, "Fontes", "Fonts"),
      tr(self.lang, "Locale e Região", "Locale & Region"),
      tr(self.lang, "Idioma", "Language"),
      tr(self.lang, "Sistema", "System"),
      tr(self.lang, "About", "About"),
    ]
  }

  pub fn move_home(&mut self, delta: isize) {
    self.home_selected = (self.home_selected as isize + delta).clamp(0, 5) as usize;
  }

  pub fn open_home(&mut self) {
    match self.home_selected {
      0 => self.open_settings(Page::DefaultApps),
      1 => self.open_settings(Page::Fonts),
      2 => self.open_settings(Page::LocaleRegion),
      3 => self.open_settings(Page::Language),
      4 => self.open_settings(Page::System),
      5 => self.route = Route::About,
      _ => {}
    }
  }

  pub fn open_settings(&mut self, page: Page) {
    self.settings.navigation = argvus_control_center_settings::navigation::Navigation::new(page);
    self.route = Route::Settings;
  }

  pub fn back(&mut self) {
    match self.route {
      Route::Home => {}
      Route::About => self.route = Route::Home,
      Route::Settings => {
        self.settings.back();
        if self.settings.page() == Page::Main {
          self.route = Route::Home;
        }
      }
    }
  }

  pub fn resize(&mut self, width: u16, height: u16) {
    self.width = width;
    self.height = height;
    self.settings.resize(width, height);
    self.about.resize(width, height);
  }

  pub fn help_lines(&self) -> Vec<String> {
    let global = [
      tr(self.lang, "Global", "Global"),
      tr(self.lang, "q             Sair", "q             Quit"),
      tr(self.lang, "Esc           Voltar", "Esc           Back"),
      tr(
        self.lang,
        "?             Fechar ajuda",
        "?             Close help",
      ),
      "",
    ];
    let local: &[&str] = match self.route {
      Route::Home => &[
        tr(self.lang, "Menu", "Menu"),
        tr(self.lang, "↑/↓           Navegar", "↑/↓           Navigate"),
        tr(self.lang, "→/Enter       Abrir", "→/Enter       Open"),
      ],
      Route::Settings => &[
        tr(self.lang, "Listas", "Lists"),
        tr(self.lang, "↑/↓           Navegar", "↑/↓           Navigate"),
        tr(
          self.lang,
          "Enter         Abrir/aplicar",
          "Enter         Open/apply",
        ),
        tr(self.lang, "/             Buscar", "/             Search"),
      ],
      Route::About => &[
        tr(self.lang, "About", "About"),
        tr(self.lang, "←/→           Abas", "←/→           Tabs"),
        tr(
          self.lang,
          "↑/↓           Navegar/rolar",
          "↑/↓           Navigate/scroll",
        ),
        tr(self.lang, "PgUp/PgDn    Rolar", "PgUp/PgDn    Scroll"),
        tr(
          self.lang,
          "Enter         Abrir ação",
          "Enter         Open action",
        ),
      ],
    };
    global
      .into_iter()
      .chain(local.iter().copied())
      .map(str::to_string)
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn home_navigation_is_bounded() {
    let mut app = App::new(InitialRoute::Home);
    app.move_home(-1);
    assert_eq!(app.home_selected, 0);
    app.move_home(20);
    assert_eq!(app.home_selected, 5);
  }

  #[test]
  fn entering_and_leaving_about_preserves_tab() {
    let mut app = App::new(InitialRoute::About(Tab::Credits));
    app.back();
    assert_eq!(app.route, Route::Home);
    app.home_selected = 5;
    app.open_home();
    assert_eq!(app.route, Route::About);
    assert_eq!(app.about.active_tab, Tab::Credits);
  }
}
