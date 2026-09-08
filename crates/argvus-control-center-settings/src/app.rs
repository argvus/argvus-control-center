use std::time::{Duration, Instant};

use argvus_control_center_apps::catalog::Category;

use crate::config::apps::AppsBackend;
use crate::config::fonts::{FontSettings, FontTarget, SettingKind};
use crate::i18n::{Lang, tr};
use crate::navigation::{Navigation, Page};
use crate::system::fonts::{self, FontEntry};
use crate::theme::Theme;

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 15;

#[derive(Debug, Clone, Copy)]
pub enum StatusKind {
  Success,
  Error,
}

pub struct Status {
  pub text: String,
  pub kind: StatusKind,
  created: Instant,
}

#[derive(Debug, Clone)]
pub struct Row {
  pub label: String,
  pub detail: Option<String>,
  pub current: bool,
}

pub struct App {
  pub lang: Lang,
  pub theme: Theme,
  pub navigation: Navigation,
  apps: AppsBackend,
  fonts: FontSettings,
  system_fonts: Vec<FontEntry>,
  pub search: String,
  pub searching: bool,
  pub pending_size: u16,
  pub status: Option<Status>,
  pub error_modal: Option<String>,
  pub width: u16,
  pub height: u16,
  pub viewport: usize,
}

impl App {
  pub fn new(initial: Page) -> Self {
    let lang = Lang::detect();
    Self::with_context(initial, lang, Theme::load())
  }

  pub fn with_context(initial: Page, lang: Lang, theme: Theme) -> Self {
    let (system_fonts, error_modal) = match fonts::list() {
      Ok(fonts) => (fonts, None),
      Err(error) => (Vec::new(), Some(error.to_string())),
    };
    Self {
      lang,
      theme,
      navigation: Navigation::new(initial),
      apps: AppsBackend::load(),
      fonts: FontSettings::load(),
      system_fonts,
      search: String::new(),
      searching: false,
      pending_size: 13,
      status: None,
      error_modal,
      width: 80,
      height: 24,
      viewport: 1,
    }
  }

  pub fn page(&self) -> Page {
    self.navigation.current().page
  }

  pub fn rows(&self) -> Vec<Row> {
    match self.page() {
      Page::Main => vec![
        Row::plain(tr(self.lang, "Apps Padrão", "Default Apps")),
        Row::plain(tr(self.lang, "Fontes", "Fonts")),
      ],
      Page::DefaultApps => Category::ORDER
        .into_iter()
        .map(|category| Row {
          label: category_label(self.lang, category).to_string(),
          detail: Some(self.apps.current(category)),
          current: false,
        })
        .collect(),
      Page::AppSelector(category) => {
        let current = self.apps.current(category);
        self
          .apps
          .installed(category)
          .iter()
          .filter(|app| search_matches(&self.search, &[&app.display, &app.binary]))
          .map(|app| Row {
            label: app.display.clone(),
            detail: Some(app.binary.clone()),
            current: app.binary == current,
          })
          .collect()
      }
      Page::Fonts => FontTarget::ALL
        .into_iter()
        .map(|target| {
          let font = self.fonts.get(target);
          Row {
            label: font_target_label(self.lang, target).to_string(),
            detail: Some(format!("{} {}", font.display_name(), font.size)),
            current: false,
          }
        })
        .chain(SettingKind::ALL.into_iter().map(|setting| Row {
          label: setting_label(self.lang, setting).to_string(),
          detail: Some(setting_value_label(
            self.lang,
            &self.fonts.setting_value(setting),
          )),
          current: false,
        }))
        .collect(),
      Page::FontSelector(target) => {
        let current = self.fonts.get(target);
        self
          .system_fonts
          .iter()
          .filter(|font| search_matches(&self.search, &[&font.family, &font.style]))
          .map(|font| Row {
            label: font.display_name(),
            detail: None,
            current: font.family == current.family && font.style == current.style,
          })
          .collect()
      }
      Page::SettingSelector(setting) => self.setting_options(setting),
    }
  }

  pub fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Configurações ARGVUS", "ARGVUS Settings");
    match self.page() {
      Page::Main => root.to_string(),
      Page::DefaultApps => format!("{root} > {}", tr(self.lang, "Apps Padrão", "Default Apps")),
      Page::AppSelector(category) => format!(
        "{root} > {} > {}",
        tr(self.lang, "Apps Padrão", "Default Apps"),
        category_label(self.lang, category)
      ),
      Page::Fonts => format!("{root} > {}", tr(self.lang, "Fontes", "Fonts")),
      Page::FontSelector(target) => format!(
        "{root} > {} > {}",
        tr(self.lang, "Fontes", "Fonts"),
        font_target_label(self.lang, target)
      ),
      Page::SettingSelector(setting) => format!(
        "{root} > {} > {}",
        tr(self.lang, "Fontes", "Fonts"),
        setting_label(self.lang, setting)
      ),
    }
  }

  pub fn footer(&self) -> &'static str {
    if self.searching {
      return tr(
        self.lang,
        "Digite para buscar   Enter Aplicar   Esc Cancelar",
        "Type to search   Enter Apply   Esc Cancel",
      );
    }
    if self.width < 100 {
      return match self.page() {
        Page::Main => tr(
          self.lang,
          "↑/↓ Nav   →/Enter Abrir   ? Ajuda   q Sair",
          "↑/↓ Nav   →/Enter Open   ? Help   q Quit",
        ),
        Page::DefaultApps | Page::Fonts => tr(
          self.lang,
          "↑/↓ Nav   →/Enter Abrir   Esc Voltar   ?",
          "↑/↓ Nav   →/Enter Open   Esc Back   ?",
        ),
        Page::FontSelector(_) => tr(
          self.lang,
          "↑/↓ Nav  +/- Tam  Enter Aplicar  / Busca  Esc  ?",
          "↑/↓ Nav  +/- Size  Enter Apply  / Search  Esc  ?",
        ),
        Page::AppSelector(_) => tr(
          self.lang,
          "↑/↓ Nav  Enter Aplicar  / Busca  Esc Voltar  ?",
          "↑/↓ Nav  Enter Apply  / Search  Esc Back  ?",
        ),
        Page::SettingSelector(_) => tr(
          self.lang,
          "↑/↓ Nav  Enter Aplicar  Esc Voltar  ?",
          "↑/↓ Nav  Enter Apply  Esc Back  ?",
        ),
      };
    }
    match self.page() {
      Page::Main => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ? Ajuda   q Sair",
        "↑/↓ Navigate   →/Enter Open   ? Help   q Quit",
      ),
      Page::DefaultApps | Page::Fonts => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   ←/Esc Back   ? Help",
      ),
      Page::FontSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   +/- Tamanho   Enter Aplicar   / Buscar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   +/- Size   Enter Apply   / Search   ←/Esc Back   ? Help",
      ),
      Page::AppSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   / Buscar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   / Search   ←/Esc Back   ? Help",
      ),
      Page::SettingSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   ←/Esc Back   ? Help",
      ),
    }
  }

  pub fn move_selection(&mut self, delta: isize) {
    let count = self.rows().len();
    let location = self.navigation.current_mut();
    if count == 0 {
      location.selected = 0;
      location.scroll = 0;
      return;
    }
    location.selected = (location.selected as isize + delta).clamp(0, count as isize - 1) as usize;
    self.ensure_visible(count);
  }

  pub fn open_or_apply(&mut self) {
    if self.error_modal.take().is_some() {
      return;
    }
    let selected = self.navigation.current().selected;
    match self.page() {
      Page::Main => match selected {
        0 => self.navigation.push(Page::DefaultApps),
        1 => self.navigation.push(Page::Fonts),
        _ => {}
      },
      Page::DefaultApps => {
        if let Some(category) = Category::ORDER.get(selected).copied() {
          self.navigation.push(Page::AppSelector(category));
          self.select_current();
        }
      }
      Page::AppSelector(category) => {
        let apps: Vec<(String, String)> = self
          .apps
          .installed(category)
          .iter()
          .filter(|app| search_matches(&self.search, &[&app.display, &app.binary]))
          .map(|app| (app.display.clone(), app.binary.clone()))
          .collect();
        if let Some((display, binary)) = apps.get(selected) {
          match self.apps.set_default(category, binary) {
            Ok(()) => self.success(format!(
              "{}: {display}",
              tr(
                self.lang,
                "Aplicativo padrão alterado",
                "Default application changed"
              )
            )),
            Err(error) => self.fail(error),
          }
        }
      }
      Page::Fonts => {
        if let Some(target) = FontTarget::ALL.get(selected).copied() {
          self.pending_size = self.fonts.get(target).size;
          self.navigation.push(Page::FontSelector(target));
          self.select_current();
        } else if let Some(setting) = SettingKind::ALL
          .get(selected.saturating_sub(FontTarget::ALL.len()))
          .copied()
        {
          self.navigation.push(Page::SettingSelector(setting));
          self.select_current();
        }
      }
      Page::FontSelector(target) => {
        let fonts: Vec<FontEntry> = self
          .system_fonts
          .iter()
          .filter(|font| search_matches(&self.search, &[&font.family, &font.style]))
          .cloned()
          .collect();
        if let Some(font) = fonts.get(selected) {
          match self.fonts.apply_font(target, font, self.pending_size) {
            Ok(()) => self.success(format!(
              "{}: {} {}",
              tr(self.lang, "Fonte aplicada", "Font applied"),
              font.display_name(),
              self.pending_size
            )),
            Err(error) => self.fail(error),
          }
        }
      }
      Page::SettingSelector(setting) => {
        if let Some(row) = self.setting_options(setting).get(selected) {
          let value = row.detail.as_deref().unwrap_or(&row.label).to_string();
          match self.fonts.apply_setting(setting, &value) {
            Ok(()) => {
              self.success(tr(self.lang, "Configuração aplicada", "Setting applied").to_string())
            }
            Err(error) => self.fail(error),
          }
        }
      }
    }
  }

  pub fn back(&mut self) {
    if self.searching || !self.search.is_empty() {
      self.searching = false;
      self.search.clear();
      self.navigation.current_mut().selected = 0;
      self.navigation.current_mut().scroll = 0;
      self.select_current();
      return;
    }
    self.navigation.back();
  }

  pub fn begin_search(&mut self) {
    if matches!(self.page(), Page::AppSelector(_) | Page::FontSelector(_)) {
      self.searching = true;
      self.search.clear();
      self.navigation.current_mut().selected = 0;
      self.navigation.current_mut().scroll = 0;
    }
  }

  pub fn push_search(&mut self, character: char) {
    self.search.push(character);
    self.navigation.current_mut().selected = 0;
    self.navigation.current_mut().scroll = 0;
  }

  pub fn pop_search(&mut self) {
    self.search.pop();
    self.navigation.current_mut().selected = 0;
    self.navigation.current_mut().scroll = 0;
  }

  pub fn adjust_size(&mut self, delta: i16) {
    if matches!(self.page(), Page::FontSelector(_)) && !self.searching {
      self.pending_size = (self.pending_size as i16 + delta).clamp(8, 32) as u16;
    }
  }

  pub fn resize(&mut self, width: u16, height: u16) {
    self.width = width;
    self.height = height;
  }

  pub fn too_small(&self) -> bool {
    self.width < MIN_WIDTH || self.height < MIN_HEIGHT
  }

  pub fn set_viewport(&mut self, viewport: usize) {
    self.viewport = viewport.max(1);
    self.ensure_visible(self.rows().len());
  }

  pub fn expire_status(&mut self) -> bool {
    if self
      .status
      .as_ref()
      .is_some_and(|status| status.created.elapsed() >= Duration::from_secs(4))
    {
      self.status = None;
      true
    } else {
      false
    }
  }

  fn select_current(&mut self) {
    if let Some(index) = self.rows().iter().position(|row| row.current) {
      self.navigation.current_mut().selected = index;
      self.ensure_visible(self.rows().len());
    }
  }

  fn ensure_visible(&mut self, count: usize) {
    let location = self.navigation.current_mut();
    location.selected = location.selected.min(count.saturating_sub(1));
    if location.selected < location.scroll {
      location.scroll = location.selected;
    } else if location.selected >= location.scroll + self.viewport {
      location.scroll = location.selected + 1 - self.viewport;
    }
  }

  fn setting_options(&self, setting: SettingKind) -> Vec<Row> {
    let values: Vec<&str> = match setting {
      SettingKind::Antialiasing => vec!["enabled", "disabled"],
      SettingKind::Hinting => vec!["none", "slight", "medium", "full"],
      SettingKind::Subpixel => vec!["none", "rgb", "bgr", "vrgb", "vbgr"],
      SettingKind::Dpi => vec![
        "automatic",
        "72",
        "84",
        "96",
        "108",
        "120",
        "144",
        "168",
        "192",
        "216",
        "240",
      ],
    };
    let current = self.fonts.setting_value(setting);
    values
      .into_iter()
      .map(|value| Row {
        label: setting_value_label(self.lang, value),
        detail: Some(value.to_string()),
        current: value == current,
      })
      .collect()
  }

  fn success(&mut self, text: String) {
    self.status = Some(Status {
      text,
      kind: StatusKind::Success,
      created: Instant::now(),
    });
  }

  fn fail(&mut self, error: impl ToString) {
    let error = error.to_string();
    self.status = Some(Status {
      text: error.clone(),
      kind: StatusKind::Error,
      created: Instant::now(),
    });
    self.error_modal = Some(error);
  }
}

impl Row {
  fn plain(label: &str) -> Self {
    Self {
      label: label.to_string(),
      detail: None,
      current: false,
    }
  }
}

pub fn search_matches(query: &str, values: &[&str]) -> bool {
  query.is_empty()
    || values
      .iter()
      .any(|value| value.to_lowercase().contains(&query.to_lowercase()))
}

pub fn category_label(lang: Lang, category: Category) -> &'static str {
  match lang {
    Lang::Pt => category.title_pt(),
    Lang::En => category.title(),
  }
}

pub fn font_target_label(lang: Lang, target: FontTarget) -> &'static str {
  match target {
    FontTarget::Taskbar => tr(lang, "Fonte da Taskbar", "Taskbar Font"),
    FontTarget::Sysinfo => tr(lang, "Fonte do SysInfo", "SysInfo Font"),
    FontTarget::ControlPanel => tr(lang, "Fonte do Painel de Controle", "Control Panel Font"),
    FontTarget::System => tr(lang, "Fonte do Sistema", "System Font"),
    FontTarget::Apps => tr(lang, "Fonte dos Aplicativos", "Applications Font"),
    FontTarget::Terminal => tr(lang, "Fonte do Terminal", "Terminal Font"),
    FontTarget::Browser => tr(lang, "Fonte do Navegador", "Browser Font"),
  }
}

pub fn setting_label(lang: Lang, setting: SettingKind) -> &'static str {
  match setting {
    SettingKind::Antialiasing => tr(lang, "Suavização", "Antialiasing"),
    SettingKind::Hinting => "Hinting",
    SettingKind::Subpixel => tr(lang, "Ordem de subpixel", "Subpixel order"),
    SettingKind::Dpi => "DPI",
  }
}

fn setting_value_label(lang: Lang, value: &str) -> String {
  match value {
    "enabled" => tr(lang, "ativada", "enabled").to_string(),
    "disabled" => tr(lang, "desativada", "disabled").to_string(),
    "none" => tr(lang, "nenhum", "none").to_string(),
    "slight" => tr(lang, "leve", "slight").to_string(),
    "medium" => tr(lang, "médio", "medium").to_string(),
    "full" => tr(lang, "completo", "full").to_string(),
    "automatic" => tr(lang, "automático", "automatic").to_string(),
    _ => value.to_string(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn search_is_case_insensitive() {
    assert!(search_matches("nOtO", &["Noto Sans", "Regular"]));
    assert!(!search_matches("Roboto", &["Noto Sans"]));
  }

  #[test]
  fn labels_cover_every_backend_category() {
    for category in Category::ORDER {
      assert!(!category_label(Lang::En, category).is_empty());
      assert!(!category_label(Lang::Pt, category).is_empty());
    }
  }
}
