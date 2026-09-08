use std::process::Command;

use crate::AboutError;
use crate::i18n::{Lang, na, tr};
use crate::image::Logo;
use crate::pages::{self, Doc};
use crate::system::{self, SystemInfo};
use crate::theme::Theme;

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
  System,
  About,
  Donate,
  Credits,
  Copyright,
}

impl Tab {
  pub const ALL: [Tab; 5] = [
    Tab::System,
    Tab::About,
    Tab::Donate,
    Tab::Credits,
    Tab::Copyright,
  ];

  pub const fn index(self) -> usize {
    match self {
      Tab::System => 0,
      Tab::About => 1,
      Tab::Donate => 2,
      Tab::Credits => 3,
      Tab::Copyright => 4,
    }
  }

  pub const fn from_index(index: usize) -> Self {
    match index % Self::ALL.len() {
      0 => Tab::System,
      1 => Tab::About,
      2 => Tab::Donate,
      3 => Tab::Credits,
      _ => Tab::Copyright,
    }
  }

  pub fn label(self, lang: Lang) -> &'static str {
    match self {
      Tab::System => tr(lang, "Sistema", "System"),
      Tab::About => tr(lang, "Sobre", "About"),
      Tab::Donate => tr(lang, "Donate", "Donate"),
      Tab::Credits => tr(lang, "Créditos", "Credits"),
      Tab::Copyright => tr(lang, "Direitos autorais", "Copyright"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
  Info,
  Success,
  Error,
}

#[derive(Debug, Clone)]
pub struct Status {
  pub text: String,
  pub kind: StatusKind,
}

struct DocCache {
  tab: Tab,
  width: usize,
  selected: usize,
  doc: Doc<'static>,
}

pub struct App {
  pub lang: Lang,
  pub theme: Theme,
  pub system: SystemInfo,
  pub argvus_version: String,
  pub gtk_version: String,
  pub logo: Option<Logo>,
  pub active_tab: Tab,
  pub scroll: usize,
  pub selected: usize,
  pub status: Option<Status>,
  pub viewport: usize,
  pub width: u16,
  pub height: u16,
  cache: Option<DocCache>,
  tab_scroll: [usize; 5],
  tab_selected: [usize; 5],
}

impl App {
  pub fn new(initial_tab: Tab) -> Self {
    let lang = Lang::detect();
    Self::with_context(initial_tab, lang, Theme::load())
  }

  pub fn with_context(initial_tab: Tab, lang: Lang, theme: Theme) -> Self {
    let missing = na(lang);
    let status = Status {
      text: tr(lang, "Bem-vindo ao ARGVUS", "Welcome to ARGVUS").to_string(),
      kind: StatusKind::Info,
    };
    Self {
      lang,
      theme,
      system: SystemInfo::gather(missing),
      argvus_version: system::argvus_version(missing),
      gtk_version: system::gtk_version(missing),
      logo: Some(Logo::new()),
      active_tab: initial_tab,
      scroll: 0,
      selected: 0,
      status: Some(status),
      viewport: 0,
      width: 80,
      height: 24,
      cache: None,
      tab_scroll: [0; 5],
      tab_selected: [0; 5],
    }
  }

  #[cfg(test)]
  pub fn test() -> Self {
    let lang = Lang::Pt;
    let theme = Theme::load();
    Self {
      lang,
      theme,
      system: SystemInfo {
        hostname: "host".into(),
        os_name: "Arch Linux".into(),
        os_type: "64 bits".into(),
        distributor: "Arch Linux".into(),
        kernel: "6.14.4-arch1-1".into(),
        window_system: "Wayland".into(),
        cpu: "Intel Core i5 8 cores".into(),
        memory: "15.5 GiB".into(),
        gpus: "Intel Graphics".into(),
      },
      argvus_version: "0.4.0".into(),
      gtk_version: "4.22.4".into(),
      logo: None,
      active_tab: Tab::System,
      scroll: 0,
      selected: 0,
      status: None,
      viewport: 0,
      width: 80,
      height: 24,
      cache: None,
      tab_scroll: [0; 5],
      tab_selected: [0; 5],
    }
  }

  pub fn current_doc(&mut self) -> Doc<'static> {
    let width = self.width.max(MIN_WIDTH) as usize;
    if let Some(cache) = &self.cache
      && cache.tab == self.active_tab
      && cache.width == width
      && cache.selected == self.selected
    {
      return cache.doc.clone();
    }
    let doc = pages::doc_for(self, width, self.selected);
    self.cache = Some(DocCache {
      tab: self.active_tab,
      width,
      selected: self.selected,
      doc: doc.clone(),
    });
    doc
  }

  pub fn set_viewport(&mut self, viewport: usize) {
    self.viewport = viewport;
    self.clamp_scroll();
  }

  pub fn next_tab(&mut self) {
    self.goto_tab(Tab::from_index(self.active_tab.index() + 1));
  }

  pub fn prev_tab(&mut self) {
    let count = Tab::ALL.len();
    self.goto_tab(Tab::from_index(self.active_tab.index() + count - 1));
  }

  pub fn goto_tab(&mut self, tab: Tab) {
    if tab == self.active_tab {
      return;
    }
    let current = self.active_tab.index();
    self.tab_scroll[current] = self.scroll;
    self.tab_selected[current] = self.selected;
    self.active_tab = tab;
    self.scroll = self.tab_scroll[tab.index()];
    self.selected = self.tab_selected[tab.index()];
    self.cache = None;
    self.clamp_scroll();
  }

  pub fn move_cursor(&mut self, delta: isize) {
    let doc = self.current_doc();
    if doc.actions.is_empty() {
      self.scroll_lines(delta);
      return;
    }
    let last = doc.actions.len() as isize - 1;
    let next = (self.selected as isize + delta).clamp(0, last) as usize;
    self.selected = next;
    self.ensure_selected_visible();
  }

  pub fn scroll_lines(&mut self, delta: isize) {
    let doc = self.current_doc();
    let max = doc.height().saturating_sub(self.viewport);
    let next = self.scroll as isize + delta;
    self.scroll = next.clamp(0, max as isize) as usize;
  }

  pub fn page(&mut self, delta: isize) {
    let step = self.viewport.max(1) as isize;
    self.scroll_lines(delta * step);
  }

  pub fn home(&mut self) {
    let doc = self.current_doc();
    if doc.actions.is_empty() {
      self.scroll = 0;
      return;
    }
    self.selected = 0;
    self.scroll = 0;
  }

  pub fn end(&mut self) {
    let doc = self.current_doc();
    if doc.actions.is_empty() {
      let max = doc.height().saturating_sub(self.viewport);
      self.scroll = max;
      return;
    }
    self.selected = doc.actions.len() - 1;
    self.ensure_selected_visible();
  }

  pub fn enter(&mut self) {
    let doc = self.current_doc();
    let Some(action) = doc.actions.get(self.selected) else {
      self.status = Some(Status {
        text: tr(self.lang, "Nenhum link aqui", "No link here").to_string(),
        kind: StatusKind::Info,
      });
      return;
    };
    match open_url(&action.url) {
      Ok(()) => {
        self.status = Some(Status {
          text: tr(self.lang, "Abrindo link…", "Opening link…").to_string(),
          kind: StatusKind::Success,
        });
      }
      Err(_) => {
        self.status = Some(Status {
          text: tr(
            self.lang,
            "Falha ao abrir o link",
            "Failed to open the link",
          )
          .to_string(),
          kind: StatusKind::Error,
        });
      }
    }
  }

  pub fn ensure_selected_visible(&mut self) {
    let line = self
      .current_doc()
      .actions
      .get(self.selected)
      .map(|action| action.line)
      .unwrap_or(0);
    if self.scroll > line {
      self.scroll = line;
    }
    if self.viewport > 0 {
      let bottom = self.scroll + self.viewport;
      if line + 1 > bottom {
        self.scroll = line + 1 - self.viewport;
      }
    }
  }

  pub fn clamp_scroll(&mut self) {
    let height = self.current_doc().height();
    let max = height.saturating_sub(self.viewport);
    self.scroll = self.scroll.min(max);
  }

  pub fn resize(&mut self, width: u16, height: u16) {
    self.width = width;
    self.height = height;
  }

  pub fn too_small(&self) -> bool {
    self.width < MIN_WIDTH || self.height < MIN_HEIGHT
  }
}

fn open_url(url: &str) -> Result<(), AboutError> {
  Command::new("xdg-open")
    .arg(url)
    .spawn()
    .map(|_| ())
    .map_err(AboutError::OpenLink)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tab_switching_wraps_around() {
    let mut app = App::test();
    assert_eq!(app.active_tab, Tab::System);
    app.prev_tab();
    assert_eq!(app.active_tab, Tab::Copyright);
    app.next_tab();
    assert_eq!(app.active_tab, Tab::System);
  }

  #[test]
  fn cursor_moves_within_links() {
    let mut app = App::test();
    app.active_tab = Tab::About;
    app.cache = None;
    app.move_cursor(1);
    assert!(app.selected <= 1);
  }

  #[test]
  fn scrolls_plain_content_without_links() {
    let mut app = App::test();
    app.active_tab = Tab::Copyright;
    app.cache = None;
    app.viewport = 20;
    app.scroll_lines(1);
    assert_eq!(app.scroll, 1);
    app.end();
    let expected = app.current_doc().height().saturating_sub(app.viewport);
    assert_eq!(app.scroll, expected);
  }

  #[test]
  fn resize_updates_dimensions() {
    let mut app = App::test();
    app.resize(120, 40);
    assert_eq!((app.width, app.height), (120, 40));
  }

  #[test]
  fn each_tab_restores_its_scroll_and_selection() {
    let mut app = App::test();
    app.active_tab = Tab::About;
    app.scroll = 4;
    app.selected = 1;
    app.goto_tab(Tab::Credits);
    app.scroll = 2;
    app.goto_tab(Tab::About);
    assert_eq!(app.scroll, 4);
    assert_eq!(app.selected, 1);
    app.goto_tab(Tab::Credits);
    assert_eq!(app.scroll, 2);
  }
}
