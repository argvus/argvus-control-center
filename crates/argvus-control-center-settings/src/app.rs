use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use argvus_control_center_apps::catalog::Category;

use crate::config::apps::AppsBackend;
use crate::config::fonts::{FontSettings, FontTarget, SettingKind};
use crate::i18n::{Lang, tr};
use crate::navigation::{Navigation, Page};
use crate::system::fonts::{self, FontEntry};
use crate::system::{host, keyboard, locale, time};
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

#[derive(Debug, Clone)]
pub enum PendingAction {
  ResetApps,
  ResetApp(Category),
  ResetFonts,
  ResetFont(FontTarget),
  ResetFontSetting(SettingKind),
  ApplySystemLocales,
  SetNtp(bool),
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
  pub confirm: Option<PendingAction>,
  pub confirm_apply_selected: bool,
  pub hostname_editing: bool,
  pub hostname_input: String,
  pub width: u16,
  pub height: u16,
  pub viewport: usize,
  distro: locale::Distro,
  timezones: Vec<String>,
  datetime: time::DateTimeInfo,
  generated_locales: Vec<String>,
  locale_gen_entries: Vec<locale::LocaleEntry>,
  selected_locales: BTreeSet<String>,
  keyboard_info: keyboard::KeyboardInfo,
  keyboard_layouts: Vec<keyboard::Layout>,
  keyboard_variants: Vec<keyboard::Variant>,
  console_keymaps: Vec<String>,
  hostname: String,
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
    let locale_gen_entries =
      locale::locale_gen_entries(std::path::Path::new("/etc/locale.gen")).unwrap_or_default();
    let selected_locales = locale_gen_entries
      .iter()
      .filter(|entry| entry.enabled)
      .map(|entry| format!("{} {}", entry.locale, entry.encoding))
      .collect();
    let keyboard_info = keyboard::info();
    let hostname = host::current();
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
      confirm: None,
      confirm_apply_selected: true,
      hostname_editing: false,
      hostname_input: hostname.clone(),
      width: 80,
      height: 24,
      viewport: 1,
      distro: locale::distro(),
      timezones: time::list_timezones(),
      datetime: time::datetime_info(),
      generated_locales: locale::generated_locales(),
      locale_gen_entries,
      selected_locales,
      keyboard_variants: keyboard::variants(&keyboard_info.x11_layout),
      keyboard_info,
      keyboard_layouts: keyboard::layouts(),
      console_keymaps: keyboard::console_keymaps(),
      hostname,
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
        Row::plain(tr(self.lang, "Locale e Região", "Locale & Region")),
        Row::plain(tr(self.lang, "Idioma", "Language")),
        Row::plain(tr(self.lang, "Sistema", "System")),
      ],
      Page::DefaultApps => Category::ORDER
        .into_iter()
        .map(|category| Row {
          label: category_label(self.lang, category).to_string(),
          detail: Some(
            self.default_detail(self.apps.current(category), self.apps.is_default(category)),
          ),
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
      Page::LocaleRegion => vec![
        Row::plain(tr(self.lang, "Fuso horário", "Time Zone")),
        Row::plain(tr(self.lang, "Data e hora", "Date & Time")),
        Row::plain(tr(self.lang, "Locale regional", "Regional Locale")),
        Row::plain(tr(self.lang, "Locales do sistema", "System Locales")),
        Row::plain("Encoding"),
        Row::plain(tr(self.lang, "Teclado", "Keyboard")),
      ],
      Page::TimeZone => self
        .timezones
        .iter()
        .filter(|zone| search_matches(&self.search, &[zone]))
        .map(|zone| Row {
          label: zone.clone(),
          detail: None,
          current: *zone == self.datetime.time_zone,
        })
        .collect(),
      Page::DateTime => vec![
        Row {
          label: tr(self.lang, "Data/hora local", "Local date/time").to_string(),
          detail: Some(self.datetime.local_time.clone()),
          current: false,
        },
        Row {
          label: tr(self.lang, "Fuso horário", "Time Zone").to_string(),
          detail: Some(self.datetime.time_zone.clone()),
          current: false,
        },
        Row {
          label: tr(
            self.lang,
            "Data e hora automáticas (NTP)",
            "Automatic date & time (NTP)",
          )
          .to_string(),
          detail: Some(enabled_label(self.lang, self.datetime.ntp.unwrap_or(false)).to_string()),
          current: false,
        },
        Row {
          label: "RTC".to_string(),
          detail: Some(
            if self.datetime.rtc_local.unwrap_or(false) {
              tr(self.lang, "local", "local")
            } else {
              "UTC"
            }
            .to_string(),
          ),
          current: false,
        },
      ],
      Page::RegionalLocale => {
        let current = locale::current_lang();
        self
          .generated_locales
          .iter()
          .filter(|value| search_matches(&self.search, &[value]))
          .map(|value| Row {
            label: value.clone(),
            detail: None,
            current: *value == current,
          })
          .collect()
      }
      Page::SystemLocales => self
        .locale_gen_entries
        .iter()
        .filter(|entry| search_matches(&self.search, &[&entry.locale, &entry.encoding]))
        .map(|entry| {
          let key = format!("{} {}", entry.locale, entry.encoding);
          Row {
            label: format!(
              "[{}] {}",
              if self.selected_locales.contains(&key) {
                "✓"
              } else {
                " "
              },
              key
            ),
            detail: None,
            current: entry.enabled,
          }
        })
        .collect(),
      Page::Encoding => vec![Row {
        label: tr(
          self.lang,
          "Codificação de caracteres atual",
          "Current character encoding",
        )
        .to_string(),
        detail: Some(locale::encoding_from_locale(&locale::current_lang())),
        current: false,
      }],
      Page::Keyboard => vec![
        Row {
          label: tr(self.lang, "Layout", "Layout").to_string(),
          detail: Some(non_empty(&self.keyboard_info.x11_layout)),
          current: false,
        },
        Row {
          label: tr(self.lang, "Variante", "Variant").to_string(),
          detail: Some(non_empty(&self.keyboard_info.x11_variant)),
          current: false,
        },
        Row {
          label: tr(self.lang, "Modelo", "Model").to_string(),
          detail: Some(non_empty(&self.keyboard_info.x11_model)),
          current: false,
        },
        Row {
          label: tr(self.lang, "Opções", "Options").to_string(),
          detail: Some(non_empty(&self.keyboard_info.x11_options)),
          current: false,
        },
        Row {
          label: tr(self.lang, "Keymap do console", "Console Keymap").to_string(),
          detail: Some(non_empty(&self.keyboard_info.console_keymap)),
          current: false,
        },
        Row {
          label: "Hyprland XKB".to_string(),
          detail: Some(format!(
            "{} {} {}",
            self.keyboard_info.hypr_layout,
            self.keyboard_info.hypr_variant,
            self.keyboard_info.hypr_options
          )),
          current: false,
        },
      ],
      Page::KeyboardLayout => self
        .keyboard_layouts
        .iter()
        .filter(|layout| search_matches(&self.search, &[&layout.code, &layout.description]))
        .map(|layout| Row {
          label: layout.code.clone(),
          detail: Some(layout.description.clone()),
          current: layout.code == self.keyboard_info.x11_layout,
        })
        .collect(),
      Page::KeyboardVariant => self
        .keyboard_variants
        .iter()
        .filter(|variant| search_matches(&self.search, &[&variant.code, &variant.description]))
        .map(|variant| Row {
          label: if variant.code.is_empty() {
            tr(self.lang, "Padrão", "Default").to_string()
          } else {
            variant.code.clone()
          },
          detail: Some(variant.description.clone()),
          current: variant.code == self.keyboard_info.x11_variant,
        })
        .collect(),
      Page::ConsoleKeymap => self
        .console_keymaps
        .iter()
        .filter(|keymap| search_matches(&self.search, &[keymap]))
        .map(|keymap| Row {
          label: keymap.clone(),
          detail: None,
          current: *keymap == self.keyboard_info.console_keymap,
        })
        .collect(),
      Page::Language => vec![
        Row {
          label: "English".to_string(),
          detail: None,
          current: self.lang == Lang::En,
        },
        Row {
          label: "Português".to_string(),
          detail: None,
          current: self.lang == Lang::Pt,
        },
      ],
      Page::System => vec![Row::plain(tr(self.lang, "Hostname", "Hostname"))],
      Page::Hostname => vec![Row {
        label: if self.hostname_editing {
          tr(self.lang, "Novo hostname", "New hostname").to_string()
        } else {
          tr(self.lang, "Hostname atual", "Current hostname").to_string()
        },
        detail: Some(if self.hostname_editing {
          format!("{}_", self.hostname_input)
        } else {
          self.hostname.clone()
        }),
        current: false,
      }],
    }
  }

  pub fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "ARGVUS Control Center", "ARGVUS Control Center");
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
      Page::LocaleRegion => format!(
        "{root} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region")
      ),
      Page::TimeZone => format!(
        "{root} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Fuso horário", "Time Zone")
      ),
      Page::DateTime => format!(
        "{root} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Data e hora", "Date & Time")
      ),
      Page::RegionalLocale => format!(
        "{root} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Locale regional", "Regional Locale")
      ),
      Page::SystemLocales => format!(
        "{root} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Locales do sistema", "System Locales")
      ),
      Page::Encoding => format!(
        "{root} > {} > Encoding",
        tr(self.lang, "Locale e Região", "Locale & Region")
      ),
      Page::Keyboard => format!(
        "{root} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Teclado", "Keyboard")
      ),
      Page::KeyboardLayout => format!(
        "{root} > {} > {} > Layout",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Teclado", "Keyboard")
      ),
      Page::KeyboardVariant => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Teclado", "Keyboard"),
        tr(self.lang, "Variante", "Variant")
      ),
      Page::ConsoleKeymap => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "Locale e Região", "Locale & Region"),
        tr(self.lang, "Teclado", "Keyboard"),
        tr(self.lang, "Keymap do console", "Console Keymap")
      ),
      Page::Language => format!("{root} > {}", tr(self.lang, "Idioma", "Language")),
      Page::System => format!("{root} > {}", tr(self.lang, "Sistema", "System")),
      Page::Hostname => format!("{root} > {} > Hostname", tr(self.lang, "Sistema", "System")),
    }
  }

  pub fn footer(&self) -> &'static str {
    if self.confirm.is_some() {
      return tr(
        self.lang,
        "Enter Confirmar   Esc Cancelar",
        "Enter Confirm   Esc Cancel",
      );
    }
    if self.hostname_editing {
      return tr(
        self.lang,
        "Digite hostname   Enter Aplicar   Esc Cancelar",
        "Type hostname   Enter Apply   Esc Cancel",
      );
    }
    if self.searching {
      return tr(
        self.lang,
        "Digite para buscar   Enter Aplicar   Esc Cancelar",
        "Type to search   Enter Apply   Esc Cancel",
      );
    }
    match self.page() {
      Page::Main => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ? Ajuda   q Sair",
        "↑/↓ Navigate   →/Enter Open   ? Help   q Quit",
      ),
      Page::DefaultApps | Page::Fonts => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   r Reset Defaults   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   r Reset Defaults   ←/Esc Back   ? Help",
      ),
      Page::AppSelector(_) | Page::FontSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   r Reset Default   / Buscar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   r Reset Default   / Search   ←/Esc Back   ? Help",
      ),
      Page::SystemLocales => tr(
        self.lang,
        "↑/↓ Navegar   Space Alternar   / Buscar   Enter Aplicar   ←/Esc Voltar",
        "↑/↓ Navigate   Space Toggle   / Search   Enter Apply   ←/Esc Back",
      ),
      Page::TimeZone
      | Page::RegionalLocale
      | Page::KeyboardLayout
      | Page::KeyboardVariant
      | Page::ConsoleKeymap => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   / Buscar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   / Search   ←/Esc Back   ? Help",
      ),
      Page::Hostname => tr(
        self.lang,
        "Enter Editar   ←/Esc Voltar   ? Ajuda",
        "Enter Edit   ←/Esc Back   ? Help",
      ),
      Page::SettingSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   r Reset Default   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   r Reset Default   ←/Esc Back   ? Help",
      ),
      _ => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   ←/Esc Back   ? Help",
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
        2 => self.navigation.push(Page::LocaleRegion),
        3 => self.navigation.push(Page::Language),
        4 => self.navigation.push(Page::System),
        _ => {}
      },
      Page::DefaultApps => {
        if let Some(category) = Category::ORDER.get(selected).copied() {
          self.navigation.push(Page::AppSelector(category));
          self.select_current();
        }
      }
      Page::AppSelector(category) => self.apply_app(category, selected),
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
      Page::FontSelector(target) => self.apply_font(target, selected),
      Page::SettingSelector(setting) => self.apply_font_setting(setting, selected),
      Page::LocaleRegion => match selected {
        0 => self.open_system_page(Page::TimeZone),
        1 => self.open_system_page(Page::DateTime),
        2 => self.open_system_page(Page::RegionalLocale),
        3 => self.open_system_page(Page::SystemLocales),
        4 => self.open_system_page(Page::Encoding),
        5 => self.open_system_page(Page::Keyboard),
        _ => {}
      },
      Page::TimeZone => self.apply_timezone(selected),
      Page::DateTime => {
        if selected == 1 {
          self.open_system_page(Page::TimeZone);
        } else if selected == 2 {
          self.open_confirm(PendingAction::SetNtp(!self.datetime.ntp.unwrap_or(false)));
        }
      }
      Page::RegionalLocale => self.apply_regional_locale(selected),
      Page::SystemLocales => self.open_confirm(PendingAction::ApplySystemLocales),
      Page::Encoding => {}
      Page::Keyboard => match selected {
        0 => self.open_system_page(Page::KeyboardLayout),
        1 => self.open_system_page(Page::KeyboardVariant),
        4 => self.open_system_page(Page::ConsoleKeymap),
        _ => {}
      },
      Page::KeyboardLayout => self.apply_keyboard_layout(selected),
      Page::KeyboardVariant => self.apply_keyboard_variant(selected),
      Page::ConsoleKeymap => self.apply_console_keymap(selected),
      Page::Language => self.apply_language(selected),
      Page::System => {
        if selected == 0 {
          self.open_system_page(Page::Hostname);
        }
      }
      Page::Hostname => {
        self.hostname_editing = true;
        self.hostname_input = self.hostname.clone();
      }
    }
  }

  pub fn reset_current(&mut self) {
    match self.page() {
      Page::DefaultApps => self.open_confirm(PendingAction::ResetApps),
      Page::AppSelector(category) => self.open_confirm(PendingAction::ResetApp(category)),
      Page::Fonts => self.open_confirm(PendingAction::ResetFonts),
      Page::FontSelector(target) => self.open_confirm(PendingAction::ResetFont(target)),
      Page::SettingSelector(setting) => {
        self.open_confirm(PendingAction::ResetFontSetting(setting));
      }
      _ => {}
    }
  }

  pub fn toggle_current(&mut self) {
    if self.page() != Page::SystemLocales {
      return;
    }
    let selected = self.navigation.current().selected;
    let rows: Vec<String> = self
      .locale_gen_entries
      .iter()
      .filter(|entry| search_matches(&self.search, &[&entry.locale, &entry.encoding]))
      .map(|entry| format!("{} {}", entry.locale, entry.encoding))
      .collect();
    if let Some(key) = rows.get(selected)
      && !self.selected_locales.remove(key)
    {
      self.selected_locales.insert(key.clone());
    }
  }

  pub fn confirm_accept(&mut self) {
    if !self.confirm_apply_selected {
      self.cancel_modal();
      return;
    }
    let Some(action) = self.confirm.take() else {
      return;
    };
    match action {
      PendingAction::ResetApps => match self.apps.reset_all() {
        Ok(()) => self.success(
          tr(
            self.lang,
            "Aplicativos padrão restaurados",
            "Default applications restored",
          )
          .to_string(),
        ),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetApp(category) => match self.apps.reset_default(category) {
        Ok(()) => self.success(format!(
          "{} → {}",
          category_label(self.lang, category),
          tr(self.lang, "padrão ARGVUS", "ARGVUS default")
        )),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetFonts => match self.fonts.reset_all() {
        Ok(()) => self.success(
          tr(
            self.lang,
            "Configurações de fonte restauradas",
            "Font settings restored",
          )
          .to_string(),
        ),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetFont(target) => match self.fonts.reset_font(target) {
        Ok(()) => self.success(format!(
          "{} → {}",
          font_target_label(self.lang, target),
          tr(self.lang, "padrão ARGVUS", "ARGVUS default")
        )),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetFontSetting(setting) => match self.fonts.reset_setting(setting) {
        Ok(()) => self.success(format!(
          "{} → {}",
          setting_label(self.lang, setting),
          tr(self.lang, "padrão ARGVUS", "ARGVUS default")
        )),
        Err(error) => self.fail(error),
      },
      PendingAction::ApplySystemLocales => self.apply_system_locales(),
      PendingAction::SetNtp(enabled) => match time::set_ntp(enabled) {
        Ok(()) => {
          self.refresh_time();
          self.success(
            tr(
              self.lang,
              "Configuração NTP aplicada",
              "NTP setting applied",
            )
            .to_string(),
          );
        }
        Err(error) => self.fail(error),
      },
    }
  }

  pub fn cancel_modal(&mut self) {
    self.confirm = None;
    self.confirm_apply_selected = true;
  }

  pub fn toggle_confirm_button(&mut self) {
    if self.confirm.is_some() {
      self.confirm_apply_selected = !self.confirm_apply_selected;
    }
  }

  pub fn hostname_input(&mut self, key: crossterm::event::KeyCode) {
    match key {
      crossterm::event::KeyCode::Esc => self.hostname_editing = false,
      crossterm::event::KeyCode::Enter => {
        let value = self.hostname_input.trim().to_string();
        match host::set(&value) {
          Ok(()) => {
            self.hostname_editing = false;
            self.hostname = host::current();
            self.success(format!(
              "{}: {value}",
              tr(self.lang, "Hostname alterado", "Hostname changed")
            ));
          }
          Err(error) => self.fail(error),
        }
      }
      crossterm::event::KeyCode::Backspace => {
        self.hostname_input.pop();
      }
      crossterm::event::KeyCode::Char(character)
        if character.is_ascii_alphanumeric() || character == '-' =>
      {
        self.hostname_input.push(character);
      }
      _ => {}
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
    self.hostname_editing = false;
    self.navigation.back();
  }

  pub fn begin_search(&mut self) {
    if matches!(
      self.page(),
      Page::AppSelector(_)
        | Page::FontSelector(_)
        | Page::TimeZone
        | Page::RegionalLocale
        | Page::SystemLocales
        | Page::KeyboardLayout
        | Page::KeyboardVariant
        | Page::ConsoleKeymap
    ) {
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

  fn open_system_page(&mut self, page: Page) {
    if page == Page::SystemLocales && self.distro.id != "arch" {
      let name = if self.distro.pretty_name.is_empty() {
        self.distro.id.clone()
      } else {
        self.distro.pretty_name.clone()
      };
      self.fail(format!(
        "{}: {name}",
        tr(
          self.lang,
          "Backend de locales implementado apenas para Arch Linux",
          "Locales backend is implemented only for Arch Linux"
        )
      ));
      return;
    }
    self.navigation.push(page);
    self.select_current();
  }

  fn open_confirm(&mut self, action: PendingAction) {
    self.confirm = Some(action);
    self.confirm_apply_selected = true;
  }

  fn apply_app(&mut self, category: Category, selected: usize) {
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

  fn apply_font(&mut self, target: FontTarget, selected: usize) {
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

  fn apply_font_setting(&mut self, setting: SettingKind, selected: usize) {
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

  fn apply_timezone(&mut self, selected: usize) {
    let zones: Vec<String> = self
      .timezones
      .iter()
      .filter(|zone| search_matches(&self.search, &[zone]))
      .cloned()
      .collect();
    if let Some(zone) = zones.get(selected) {
      match time::set_timezone(zone, &self.timezones) {
        Ok(()) => {
          self.refresh_time();
          self.success(format!(
            "{}: {zone}",
            tr(self.lang, "Fuso horário alterado", "Time zone changed")
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  fn apply_regional_locale(&mut self, selected: usize) {
    let locales: Vec<String> = self
      .generated_locales
      .iter()
      .filter(|value| search_matches(&self.search, &[value]))
      .cloned()
      .collect();
    if let Some(value) = locales.get(selected) {
      match locale::set_lang(value, &self.generated_locales) {
        Ok(()) => self.success(format!(
          "{}: {value}",
          tr(
            self.lang,
            "Locale regional alterado",
            "Regional locale changed"
          )
        )),
        Err(error) => self.fail(error),
      }
    }
  }

  fn apply_system_locales(&mut self) {
    match locale::set_system_locales(&self.locale_gen_entries, &self.selected_locales) {
      Ok(()) => {
        self.locale_gen_entries =
          locale::locale_gen_entries(std::path::Path::new("/etc/locale.gen")).unwrap_or_default();
        self.generated_locales = locale::generated_locales();
        self.success(
          tr(
            self.lang,
            "Locales gerados com sucesso",
            "Locales generated successfully",
          )
          .to_string(),
        );
      }
      Err(error) => self.fail(error),
    }
  }

  fn apply_keyboard_layout(&mut self, selected: usize) {
    let layouts: Vec<keyboard::Layout> = self
      .keyboard_layouts
      .iter()
      .filter(|layout| search_matches(&self.search, &[&layout.code, &layout.description]))
      .cloned()
      .collect();
    if let Some(layout) = layouts.get(selected) {
      match keyboard::set_x11_layout(&layout.code, &self.keyboard_layouts) {
        Ok(()) => {
          self.refresh_keyboard();
          self.success(format!(
            "{}: {}",
            tr(
              self.lang,
              "Layout de teclado alterado",
              "Keyboard layout changed"
            ),
            layout.code
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  fn apply_keyboard_variant(&mut self, selected: usize) {
    let variants: Vec<keyboard::Variant> = self
      .keyboard_variants
      .iter()
      .filter(|variant| search_matches(&self.search, &[&variant.code, &variant.description]))
      .cloned()
      .collect();
    if let Some(variant) = variants.get(selected) {
      let layout = self.keyboard_info.x11_layout.clone();
      match keyboard::set_x11_variant(&layout, &variant.code, &self.keyboard_variants) {
        Ok(()) => {
          self.refresh_keyboard();
          self.success(format!(
            "{}: {}",
            tr(self.lang, "Variante alterada", "Variant changed"),
            variant.description
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  fn apply_console_keymap(&mut self, selected: usize) {
    let keymaps: Vec<String> = self
      .console_keymaps
      .iter()
      .filter(|keymap| search_matches(&self.search, &[keymap]))
      .cloned()
      .collect();
    if let Some(keymap) = keymaps.get(selected) {
      match keyboard::set_console_keymap(keymap, &self.console_keymaps) {
        Ok(()) => {
          self.refresh_keyboard();
          self.success(format!(
            "{}: {keymap}",
            tr(
              self.lang,
              "Keymap do console alterado",
              "Console keymap changed"
            )
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  fn apply_language(&mut self, selected: usize) {
    self.lang = if selected == 1 { Lang::Pt } else { Lang::En };
    let path = crate::config::paths::argvus_config_home().join("language");
    if let Err(error) = std::fs::create_dir_all(crate::config::paths::argvus_config_home()) {
      self.fail(error);
      return;
    }
    let value = if self.lang == Lang::Pt {
      "pt_BR"
    } else {
      "en_US"
    };
    if let Err(error) = std::fs::write(path, format!("{value}\n")) {
      self.fail(error);
    } else {
      self.success(
        tr(
          self.lang,
          "Idioma da interface aplicado",
          "Interface language applied",
        )
        .to_string(),
      );
    }
  }

  fn refresh_time(&mut self) {
    self.datetime = time::datetime_info();
  }

  fn refresh_keyboard(&mut self) {
    self.keyboard_info = keyboard::info();
    self.keyboard_variants = keyboard::variants(&self.keyboard_info.x11_layout);
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

  fn default_detail(&self, value: String, default: bool) -> String {
    if default {
      format!(
        "{value} [{}]",
        tr(self.lang, "Padrão ARGVUS", "ARGVUS Default")
      )
    } else {
      value
    }
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
    FontTarget::Sysinfo => tr(lang, "Fonte do Widget Telemetria", "Widget Telemetry Font"),
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

pub fn pending_action_text(lang: Lang, action: &PendingAction) -> (String, String) {
  match action {
    PendingAction::ResetApps => (
      tr(
        lang,
        "Redefinir aplicativos padrão?",
        "Reset default applications?",
      )
      .to_string(),
      tr(
        lang,
        "Isso restaurará as escolhas definidas pelo ARGVUS.",
        "This will restore the application choices defined by ARGVUS.",
      )
      .to_string(),
    ),
    PendingAction::ResetApp(category) => (
      format!(
        "{}?",
        tr(
          lang,
          "Redefinir aplicativo padrão",
          "Reset default application"
        )
      ),
      category_label(lang, *category).to_string(),
    ),
    PendingAction::ResetFonts => (
      tr(
        lang,
        "Redefinir configurações de fonte?",
        "Reset font settings?",
      )
      .to_string(),
      tr(
        lang,
        "Isso restaurará as configurações de fonte definidas pelo ARGVUS.",
        "This will restore the font settings defined by ARGVUS.",
      )
      .to_string(),
    ),
    PendingAction::ResetFont(target) => (
      format!("{}?", tr(lang, "Redefinir fonte", "Reset font")),
      font_target_label(lang, *target).to_string(),
    ),
    PendingAction::ResetFontSetting(setting) => (
      format!("{}?", tr(lang, "Redefinir configuração", "Reset setting")),
      setting_label(lang, *setting).to_string(),
    ),
    PendingAction::ApplySystemLocales => (
      tr(
        lang,
        "Aplicar alterações de locale?",
        "Apply locale changes?",
      )
      .to_string(),
      tr(
        lang,
        "Isso atualizará /etc/locale.gen e executará locale-gen.",
        "This will update /etc/locale.gen and run locale-gen.",
      )
      .to_string(),
    ),
    PendingAction::SetNtp(enabled) => (
      tr(
        lang,
        "Alterar data/hora automática?",
        "Change automatic date & time?",
      )
      .to_string(),
      enabled_label(lang, *enabled).to_string(),
    ),
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

fn enabled_label(lang: Lang, enabled: bool) -> &'static str {
  if enabled {
    tr(lang, "Ativado", "Enabled")
  } else {
    tr(lang, "Desativado", "Disabled")
  }
}

fn non_empty(value: &str) -> String {
  if value.is_empty() {
    "-".to_string()
  } else {
    value.to_string()
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
