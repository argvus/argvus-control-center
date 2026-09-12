use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use argvus_control_center_apps::catalog::Category;

use crate::config::apps::AppsBackend;
use crate::config::fonts::{FontSettings, FontTarget, SettingKind};
use crate::i18n::{Lang, na, tr};
use crate::navigation::{Navigation, Page};
use crate::system::fonts::{self, FontEntry};
use crate::system::{host, keyboard, locale, time};
use crate::theme::Theme;
use argvus_control_center_core::config::AppConfig;
use argvus_control_center_core::{
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedRequest, SystemSettingsOperation},
  process::{LiveProcess, SystemProcessRunner},
};

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 15;

const LANGUAGE_INFO_ROWS: usize = 3;
const REGIONAL_LOCALE_INFO_ROWS: usize = 2;

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
  Administration(String),
  ResetApps,
  ResetApp(Category),
  ResetFonts,
  ResetFont(FontTarget),
  ResetFontSetting(SettingKind),
  ApplySystemLocales,
  SetNtp(bool),
}

pub struct App {
  pub admin: crate::administration::Administration,
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
  button_from: Option<usize>,
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
  jobs: JobManager,
  task_job: Option<JobHandle<Result<String, String>>>,
  pub task_live: Option<LiveProcess>,
  pub task_open: bool,
  pub task_scroll: u16,
  pub task_follow: bool,
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
    let mut app = Self {
      admin: crate::administration::Administration::new(initial),
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
      confirm_apply_selected: false,
      hostname_editing: false,
      hostname_input: hostname.clone(),
      width: 80,
      height: 24,
      viewport: 1,
      button_from: None,
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
      jobs: JobManager::default(),
      task_job: None,
      task_live: None,
      task_open: false,
      task_scroll: 0,
      task_follow: false,
    };
    if initial != Page::Main {
      app.select_current();
    }
    app
  }

  pub fn page(&self) -> Page {
    self.navigation.current().page
  }

  pub fn rows(&self) -> Vec<Row> {
    if crate::administration::is_page(self.page()) {
      return self
        .admin
        .rows_filtered(self.page(), self.lang, &self.search);
    }
    match self.page() {
      Page::Main => vec![
        Row::plain(tr(self.lang, "Apps Padrão", "Default Apps")),
        Row::plain(tr(self.lang, "Fontes", "Fonts")),
        Row::plain(tr(self.lang, "Locale e Região", "Locale & Region")),
        Row::plain(tr(self.lang, "Sistema", "System")),
      ],
      Page::DefaultApps => Category::ORDER
        .into_iter()
        .map(|category| Row {
          label: format!(
            "{} {}",
            AppConfig::icon(category_icon(category)),
            category_label(self.lang, category)
          ),
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
            label: format!(
              "{} {}",
              AppConfig::icon(font_target_icon(target)),
              font_target_label(self.lang, target)
            ),
            detail: Some(format!("{} · {}", font.display_name(), font.size)),
            current: false,
          }
        })
        .chain(SettingKind::ALL.into_iter().map(|setting| Row {
          label: format!(
            "{} {}",
            AppConfig::icon(setting_icon(setting)),
            setting_label(self.lang, setting)
          ),
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
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🌍"),
            tr(self.lang, "Fuso horário", "Time Zone")
          ),
          detail: Some(non_empty(&self.datetime.time_zone)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🕒"),
            tr(self.lang, "Data e hora", "Date & Time")
          ),
          detail: Some(non_empty(&self.datetime.local_time)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🌐"),
            tr(self.lang, "Locale regional", "Regional Locale")
          ),
          detail: Some(locale::current_lang()),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🗂️"),
            tr(self.lang, "Locales do sistema", "System Locales")
          ),
          detail: Some(format!(
            "{} / {}",
            self.selected_locales.len(),
            self.locale_gen_entries.len()
          )),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("⌨️"),
            tr(self.lang, "Teclado", "Keyboard")
          ),
          detail: Some(format!(
            "{}  ·  {}",
            non_empty(&self.keyboard_info.x11_layout),
            non_empty(&self.keyboard_info.x11_variant)
          )),
          current: false,
        },
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
          label: format!(
            "{} {}",
            AppConfig::icon("🕒"),
            tr(self.lang, "Data/hora local", "Local date/time")
          ),
          detail: Some(self.datetime.local_time.clone()),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🌍"),
            tr(self.lang, "Fuso horário", "Time Zone")
          ),
          detail: Some(self.datetime.time_zone.clone()),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🛰️"),
            tr(
              self.lang,
              "Data e hora automáticas (NTP)",
              "Automatic date & time (NTP)",
            )
          ),
          detail: Some(enabled_label(self.lang, self.datetime.ntp.unwrap_or(false)).to_string()),
          current: false,
        },
        Row {
          label: format!("{} RTC", AppConfig::icon("🔋")),
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
        let mut rows = vec![
          Row {
            label: tr(self.lang, "Locale atual", "Current locale").to_string(),
            detail: Some(non_empty(&current)),
            current: false,
          },
          Row {
            label: tr(self.lang, "Codificação", "Encoding").to_string(),
            detail: Some(non_empty(&locale::encoding_from_locale(&current))),
            current: false,
          },
        ];
        rows.extend(
          self
            .generated_locales
            .iter()
            .filter(|value| search_matches(&self.search, &[value]))
            .map(|value| Row {
              label: value.clone(),
              detail: None,
              current: *value == current,
            }),
        );
        rows
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
      Page::Keyboard => vec![
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("⌨️"),
            tr(self.lang, "Layout", "Layout")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_layout)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🔠"),
            tr(self.lang, "Variante", "Variant")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_variant)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🖮"),
            tr(self.lang, "Modelo", "Model")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_model)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("⚙️"),
            tr(self.lang, "Opções", "Options")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_options)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon("🖥️"),
            tr(self.lang, "Keymap do console", "Console Keymap")
          ),
          detail: Some(non_empty(&self.keyboard_info.console_keymap)),
          current: false,
        },
        Row {
          label: format!("{} Hyprland XKB", AppConfig::icon("🌿")),
          detail: Some(format!(
            "{} {} {}",
            non_empty(&self.keyboard_info.hypr_layout),
            non_empty(&self.keyboard_info.hypr_variant),
            non_empty(&self.keyboard_info.hypr_options)
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
      Page::Language => self.language_rows(),
      Page::System => self.system_rows(),
      Page::Firewall
      | Page::Users
      | Page::UserList
      | Page::SystemUsers
      | Page::User
      | Page::CreateUser
      | Page::UserGroups
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup
      | Page::Groups
      | Page::GroupList
      | Page::SystemGroups
      | Page::Group
      | Page::GroupMembers
      | Page::CreateGroup => unreachable!(),
      Page::Hostname => vec![Row {
        label: tr(self.lang, "Hostname atual", "Current hostname").to_string(),
        detail: Some(self.hostname.clone()),
        current: false,
      }],
    }
  }

  fn system_rows(&self) -> Vec<Row> {
    let hostname = if self.hostname.is_empty() {
      na(self.lang).to_string()
    } else {
      self.hostname.clone()
    };
    let accounts_loaded = self.admin.is_loaded();
    let users = if accounts_loaded {
      self.admin.users(false).len().to_string()
    } else {
      na(self.lang).to_string()
    };
    let groups = if accounts_loaded {
      self.admin.groups(false).len().to_string()
    } else {
      na(self.lang).to_string()
    };
    vec![
      Row {
        label: format!("{} Hostname", AppConfig::icon("🖥️")),
        detail: Some(hostname),
        current: false,
      },
      Row {
        label: format!(
          "{} {}",
          AppConfig::icon("👤"),
          tr(self.lang, "Usuários", "Users")
        ),
        detail: Some(users),
        current: false,
      },
      Row {
        label: format!(
          "{} {}",
          AppConfig::icon("👥"),
          tr(self.lang, "Grupos", "Groups")
        ),
        detail: Some(groups),
        current: false,
      },
    ]
  }

  fn language_rows(&self) -> Vec<Row> {
    let current_lang = locale::current_lang();
    let current_language = match self.lang {
      Lang::En => "English (US) · en_US",
      Lang::Pt => "Português (Brasil) · pt_BR",
    };
    let in_use = tr(self.lang, "Em uso", "In use");
    let mut rows = vec![
      Row {
        label: tr(self.lang, "Idioma atual", "Current language").to_string(),
        detail: Some(current_language.to_string()),
        current: false,
      },
      Row {
        label: tr(self.lang, "Locale regional", "Regional locale").to_string(),
        detail: Some(non_empty(&current_lang)),
        current: false,
      },
      Row {
        label: tr(self.lang, "Codificação", "Encoding").to_string(),
        detail: Some(non_empty(&locale::encoding_from_locale(&current_lang))),
        current: false,
      },
    ];
    for (lang, flag, name, detail) in [
      (Lang::En, "🇺🇸", "English", "English (US) · en_US"),
      (Lang::Pt, "🇧🇷", "Português", "Português (Brasil) · pt_BR"),
    ] {
      let badge = if self.lang == lang {
        format!(" · {in_use}")
      } else {
        String::new()
      };
      rows.push(Row {
        label: format!("{}  {}", AppConfig::icon(flag), name),
        detail: Some(format!("{detail}{badge}")),
        current: self.lang == lang,
      });
    }
    rows
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
      Page::Firewall => format!("{root} > {} > Firewall", tr(self.lang, "Rede", "Network")),
      Page::Users => format!(
        "{root} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Usuários", "Users")
      ),
      Page::UserList => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Usuários", "Users"),
        tr(self.lang, "Listar", "List")
      ),
      Page::SystemUsers => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Usuários", "Users"),
        tr(self.lang, "Contas do sistema", "System accounts")
      ),
      Page::User
      | Page::CreateUser
      | Page::UserGroups
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup => format!(
        "{root} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Usuários", "Users")
      ),
      Page::Groups => format!(
        "{root} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Grupos", "Groups")
      ),
      Page::GroupList => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Grupos", "Groups"),
        tr(self.lang, "Listar", "List")
      ),
      Page::SystemGroups => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Grupos", "Groups"),
        tr(self.lang, "Contas do sistema", "System accounts")
      ),
      Page::Group | Page::GroupMembers | Page::CreateGroup => format!(
        "{root} > {} > {}",
        tr(self.lang, "Sistema", "System"),
        tr(self.lang, "Grupos", "Groups")
      ),
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
      Page::DefaultApps => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   Tab Ações   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   Tab Actions   ←/Esc Back   ? Help",
      ),
      Page::Fonts => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   Tab Ações   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   Tab Actions   ←/Esc Back   ? Help",
      ),
      Page::AppSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   ←/→ Mover   Enter Ativar   / Buscar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   ←/→ Move   Enter Activate   / Search   ←/Esc Back   ? Help",
      ),
      Page::FontSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   +/- Tamanho   / Buscar   Tab Ações   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   +/- Size   / Search   Tab Actions   ←/Esc Back   ? Help",
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
      Page::Language => tr(
        self.lang,
        "↑/↓ Navegar   Enter Aplicar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Enter Apply   ←/Esc Back   ? Help",
      ),
      Page::System => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   r Refresh   ←/Esc Back   ? Help",
      ),
      Page::SettingSelector(_) => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   ←/→ Mover   Enter Ativar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   ←/→ Move   Enter Activate   ←/Esc Back   ? Help",
      ),
      Page::UserList | Page::SystemUsers | Page::GroupList | Page::SystemGroups => tr(
        self.lang,
        "↑/↓ Navegar   / Buscar   →/Enter Abrir   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   / Search   →/Enter Open   ←/Esc Back   ? Help",
      ),
      Page::User | Page::CreateUser | Page::CreateGroup | Page::Group | Page::Firewall => tr(
        self.lang,
        "↑/↓ Campos   Tab Alternar   ←/→ Ações   Enter Ativar   Esc Voltar   ? Ajuda",
        "↑/↓ Fields   Tab Switch   ←/→ Actions   Enter Activate   Esc Back   ? Help",
      ),
      _ => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   ←/Esc Back   ? Help",
      ),
    }
  }

  pub fn move_selection(&mut self, delta: isize) {
    let count = self.item_count();
    if count == 0 {
      let location = self.navigation.current_mut();
      location.selected = 0;
      location.scroll = 0;
      return;
    }
    let step = delta.signum();
    if step == 0 {
      return;
    }
    let page = self.page();
    let current = self.navigation.current().selected;
    let rows = if crate::administration::is_page(page) || self.reset_page() {
      self.rows().len()
    } else {
      count
    };
    let buttons = self.page_buttons().len();
    if buttons > 0 && current >= rows {
      return;
    }
    let (lo, hi) = if buttons > 0 {
      (0, rows.saturating_sub(1))
    } else {
      (0, count - 1)
    };
    if lo > hi {
      self.navigation.current_mut().selected = 0;
      self.ensure_visible(count);
      return;
    }
    let mut selected = current;
    let mut candidate = current;
    loop {
      let probe = (candidate as isize + step).clamp(lo as isize, hi as isize) as usize;
      if probe == candidate {
        break;
      }
      candidate = probe;
      if self.row_selectable(candidate) {
        selected = candidate;
        break;
      }
    }
    self.navigation.current_mut().selected = selected;
    self.ensure_visible(count);
  }

  pub fn on_buttons(&self) -> bool {
    let buttons = self.page_buttons();
    let rows = self.rows().len();
    !buttons.is_empty() && self.navigation.current().selected >= rows
  }

  pub fn row_selectable(&self, index: usize) -> bool {
    if self.page() == Page::DateTime {
      return (index == 0 && !self.datetime.ntp.unwrap_or(false)) || index == 2;
    }
    if self.page() == Page::Keyboard {
      return matches!(index, 0 | 1 | 4);
    }
    if self.page() == Page::Language {
      return index >= LANGUAGE_INFO_ROWS && index < self.rows().len();
    }
    if self.page() == Page::RegionalLocale {
      return index >= REGIONAL_LOCALE_INFO_ROWS && index < self.rows().len();
    }
    let page = self.page();
    if crate::administration::is_page(page) {
      let rows = self.rows();
      let buttons = self.page_buttons().len();
      if index >= rows.len() {
        return index < rows.len() + buttons;
      }
      return rows.get(index).is_some() && self.admin.row_selectable(page, index);
    }
    let rows = self.rows();
    if self.reset_page() {
      let buttons = self.page_buttons().len();
      if index >= rows.len() {
        return index < rows.len() + buttons;
      }
    }
    rows.get(index).is_some()
  }

  pub fn normalize_selection(&mut self) {
    let count = self.item_count();
    if count == 0 {
      return;
    }
    if !self.row_selectable(self.navigation.current().selected)
      && let Some(index) = (0..count).find(|index| self.row_selectable(*index))
    {
      self.navigation.current_mut().selected = index;
      self.ensure_visible(count);
    }
  }

  pub fn item_count(&self) -> usize {
    if crate::administration::is_page(self.page()) {
      let rows = self
        .admin
        .rows_filtered(self.page(), self.lang, &self.search)
        .len();
      rows + self.admin.buttons(self.page(), self.lang).len()
    } else {
      self.rows().len() + self.page_buttons().len()
    }
  }

  pub fn cycle_selection(&mut self, delta: isize) {
    if delta == 0 {
      return;
    }
    let page = self.page();
    if !crate::administration::is_page(page) && !self.reset_page() {
      return;
    }
    let rows = self.rows().len();
    let buttons = self.page_buttons().len();
    if rows == 0 || buttons == 0 {
      return;
    }
    let current = self.navigation.current().selected;
    if current >= rows {
      let field = match self.button_from {
        Some(index) if index < rows && self.row_selectable(index) => index,
        _ => (0..rows)
          .find(|index| self.row_selectable(*index))
          .unwrap_or(0),
      };
      self.button_from = None;
      self.navigation.current_mut().selected = field;
    } else {
      self.button_from = Some(current);
      let button = if delta > 0 { 0 } else { buttons - 1 };
      self.navigation.current_mut().selected = rows + button;
    }
    self.ensure_visible(self.item_count());
  }

  pub fn move_button(&mut self, delta: isize) {
    let page = self.page();
    if (!crate::administration::is_page(page) && !self.reset_page()) || delta == 0 {
      return;
    }
    let rows = self.rows().len();
    let buttons = self.page_buttons().len();
    if buttons == 0 {
      return;
    }
    let current = self.navigation.current().selected;
    if current < rows {
      return;
    }
    let mut index = current - rows;
    if delta > 0 {
      index = (index + 1) % buttons;
    } else {
      index = (index + buttons - 1) % buttons;
    }
    self.navigation.current_mut().selected = rows + index;
    self.ensure_visible(self.item_count());
  }

  pub fn admin_buttons(&self, page: Page) -> Vec<crate::administration::Button> {
    self.admin.buttons(page, self.lang)
  }

  pub fn reset_page(&self) -> bool {
    matches!(
      self.page(),
      Page::DefaultApps
        | Page::Fonts
        | Page::AppSelector(_)
        | Page::FontSelector(_)
        | Page::SettingSelector(_)
    )
  }

  pub fn page_buttons(&self) -> Vec<crate::administration::Button> {
    let page = self.page();
    if crate::administration::is_page(page) {
      self.admin.buttons(page, self.lang)
    } else if self.reset_page() {
      vec![crate::administration::Button::new(
        tr(self.lang, "Restaurar padrões", "Reset Defaults"),
        crate::administration::ButtonKind::Secondary,
      )]
    } else {
      Vec::new()
    }
  }

  pub fn open_or_apply(&mut self) {
    if crate::administration::is_page(self.page()) {
      self.admin_open();
      return;
    }
    if self.reset_page() && self.on_buttons() {
      self.reset_current();
      return;
    }
    if self.error_modal.take().is_some() {
      return;
    }
    let selected = self.navigation.current().selected;
    match self.page() {
      Page::Main => match selected {
        0 => self.navigation.push(Page::DefaultApps),
        1 => self.navigation.push(Page::Fonts),
        2 => self.navigation.push(Page::LocaleRegion),
        3 => self.navigation.push(Page::System),
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
        4 => self.open_system_page(Page::Keyboard),
        _ => {}
      },
      Page::TimeZone => self.apply_timezone(selected),
      Page::DateTime => {
        if selected == 0 && !self.datetime.ntp.unwrap_or(false) {
          self.admin.editor = Some(crate::administration::Editor::new(
            tr(self.lang, "Data/hora local", "Local date/time").into(),
            self.datetime.local_time.clone(),
            crate::administration::EditTarget::DateTime,
            false,
          ));
        } else if selected == 2 {
          self.open_confirm(PendingAction::SetNtp(!self.datetime.ntp.unwrap_or(false)));
        }
      }
      Page::RegionalLocale => self.apply_regional_locale(selected),
      Page::SystemLocales => self.open_confirm(PendingAction::ApplySystemLocales),
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
      Page::System => match selected {
        0 => self.open_system_page(Page::Hostname),
        1 => {
          self.admin.load(false);
          self.navigation.push(Page::Users);
          self.normalize_selection();
        }
        2 => {
          self.admin.load(false);
          self.navigation.push(Page::Groups);
          self.normalize_selection();
        }
        _ => {}
      },
      Page::Firewall
      | Page::Users
      | Page::UserList
      | Page::SystemUsers
      | Page::User
      | Page::CreateUser
      | Page::UserGroups
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup
      | Page::Groups
      | Page::GroupList
      | Page::SystemGroups
      | Page::Group
      | Page::GroupMembers
      | Page::CreateGroup => unreachable!(),
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

  pub fn refresh_system(&mut self) {
    self.hostname = host::current();
    self.admin.load(false);
    self.success(tr(self.lang, "Dados atualizados", "Data updated").to_string());
  }

  pub fn toggle_current(&mut self) {
    if crate::administration::is_page(self.page()) {
      self.admin_open();
      return;
    }
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
      PendingAction::Administration(_) => self.admin.submit(),
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
    self.admin.cancel_pending();
    self.confirm = None;
    self.confirm_apply_selected = false;
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
    self.normalize_selection();
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
        | Page::SystemUsers
        | Page::GroupList
        | Page::SystemGroups
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
    if let Some(result) = self.admin.poll() {
      match result {
        Ok(()) => {
          match self.admin.completed.take().as_deref() {
            Some("create") if self.page() == Page::CreateUser => {
              self.navigation.current_mut().page = Page::User;
              self.normalize_selection();
            }
            Some("delete") | Some("delete-group") => {
              self.navigation.back();
            }
            _ => {}
          }
          self.success(tr(self.lang, "Dados atualizados", "Data updated").into());
        }
        Err(error) => self.fail(error),
      }
      return true;
    }
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

  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = self.task_job.take() {
      match job.try_state() {
        JobState::Running => {
          self.task_job = Some(job);
          if self.task_open {
            if self.task_follow
              && let Some(live) = &self.task_live
            {
              self.task_scroll = task_bottom_offset(&live.output());
            }
            changed = true;
          }
        }
        JobState::Finished(Ok(Ok(_))) => {
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
          changed = true;
        }
        JobState::Finished(Ok(Err(error))) | JobState::Finished(Err(error)) => self.fail(error),
      }
    }
    changed
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
    self.confirm_apply_selected = false;
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
    if let Some(value) = locales.get(selected.saturating_sub(REGIONAL_LOCALE_INFO_ROWS)) {
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
    let valid: BTreeSet<String> = self
      .locale_gen_entries
      .iter()
      .map(|entry| format!("{} {}", entry.locale, entry.encoding))
      .collect();
    if self
      .selected_locales
      .iter()
      .any(|value| !valid.contains(value))
    {
      self.fail("selected locale is not present in /etc/locale.gen");
      return;
    }
    let executable = match std::env::current_exe() {
      Ok(path) => path.to_string_lossy().into_owned(),
      Err(error) => {
        self.fail(error.to_string());
        return;
      }
    };
    let request = match PrivilegedRequest::new(
      "locale",
      "set-generated",
      self.selected_locales.iter().cloned().collect(),
    ) {
      Ok(request) => request,
      Err(error) => {
        self.fail(error.to_string());
        return;
      }
    };
    let live = LiveProcess::new();
    self.task_live = Some(live.clone());
    self.task_open = true;
    self.task_scroll = 0;
    self.task_follow = true;
    live.push_line("$ locale-gen");
    let operation = SystemSettingsOperation::new(SystemProcessRunner, executable);
    self.task_job = Some(self.jobs.spawn(move |_| {
      Ok(match operation.execute_live(&request, &live) {
        Ok(output) if output.status == Some(0) => {
          let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
          Ok(if stdout.is_empty() {
            "ok".into()
          } else {
            stdout
          })
        }
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(error) => Err(error.to_string()),
      })
    }));
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
    self.lang = language_from_selected(selected);
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

  pub(crate) fn refresh_time(&mut self) {
    self.datetime = time::datetime_info();
  }

  fn refresh_keyboard(&mut self) {
    self.keyboard_info = keyboard::info();
    self.keyboard_variants = keyboard::variants(&self.keyboard_info.x11_layout);
  }

  pub fn select_current(&mut self) {
    if let Some(index) = self.rows().iter().position(|row| row.current) {
      self.navigation.current_mut().selected = index;
      self.ensure_visible(self.rows().len());
    } else {
      self.normalize_selection();
    }
  }

  fn ensure_visible(&mut self, count: usize) {
    let buttons = self.page_buttons().len();
    let rows = if buttons > 0 {
      self.rows().len()
    } else {
      count
    };
    let full = rows + buttons;
    let location = self.navigation.current_mut();
    location.selected = location.selected.min(full.saturating_sub(1));
    let effective = location.selected.min(rows.saturating_sub(1));
    if effective < location.scroll {
      location.scroll = effective;
    } else if effective >= location.scroll + self.viewport {
      location.scroll = effective + 1 - self.viewport;
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

  pub(crate) fn fail(&mut self, error: impl ToString) {
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

pub fn category_icon(category: Category) -> &'static str {
  match category {
    Category::Terminal => "🖥️",
    Category::FileManager => "📁",
    Category::TextEditor => "📝",
    Category::TerminalEditor => "⌨️",
    Category::Browser => "🌐",
    Category::ImageViewer => "🖼️",
    Category::PdfViewer => "📄",
    Category::VideoPlayer => "🎬",
    Category::AudioPlayer => "🎵",
    Category::Archive => "📦",
    Category::Launcher => "🚀",
  }
}

pub fn category_label(lang: Lang, category: Category) -> &'static str {
  match lang {
    Lang::Pt => category.title_pt(),
    Lang::En => category.title(),
  }
}

pub fn font_target_icon(target: FontTarget) -> &'static str {
  match target {
    FontTarget::Taskbar => "🖥️",
    FontTarget::Sysinfo => "📊",
    FontTarget::ControlPanel => "🎛️",
    FontTarget::System => "💻",
    FontTarget::Apps => "📦",
    FontTarget::Terminal => "⌨️",
    FontTarget::Browser => "🌐",
  }
}

pub fn setting_icon(setting: SettingKind) -> &'static str {
  match setting {
    SettingKind::Antialiasing => "✨",
    SettingKind::Hinting => "🔍",
    SettingKind::Subpixel => "🌈",
    SettingKind::Dpi => "📐",
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
    PendingAction::Administration(body) => (
      tr(
        lang,
        "Confirmar operação administrativa?",
        "Confirm administrative operation?",
      )
      .into(),
      body.clone(),
    ),
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

pub(crate) const TASK_POPUP_WIDTH: u16 = 100;
pub(crate) const TASK_POPUP_HEIGHT: u16 = 20;
pub(crate) const TASK_CONTENT_WIDTH: usize = TASK_POPUP_WIDTH as usize - 2;
pub(crate) const TASK_CONTENT_HEIGHT: usize = TASK_POPUP_HEIGHT as usize - 2;

pub(crate) fn task_wrapped_lines(output: &str) -> usize {
  output
    .lines()
    .map(|line| {
      let width = argvus_tui::text::display_width(line);
      if width == 0 {
        1
      } else {
        width.div_ceil(TASK_CONTENT_WIDTH)
      }
    })
    .sum()
}

pub(crate) fn task_bottom_offset(output: &str) -> u16 {
  task_wrapped_lines(output).saturating_sub(TASK_CONTENT_HEIGHT) as u16
}

fn language_from_selected(selected: usize) -> Lang {
  if selected.saturating_sub(LANGUAGE_INFO_ROWS) == 1 {
    Lang::Pt
  } else {
    Lang::En
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

  #[test]
  fn language_page_shows_status_lines_and_selectable_choices() {
    let app = App::with_context(Page::Language, Lang::En, Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].label, "Current language");
    assert!(rows[0].detail.is_some());
    assert_eq!(rows[1].label, "Regional locale");
    assert_eq!(rows[2].label, "Encoding");
    assert!(rows[3].label.starts_with('🇺'), "{}", rows[3].label);
    assert!(rows[4].label.starts_with('🇧'), "{}", rows[4].label);
    let current = rows.iter().position(|row| row.current).unwrap();
    assert!(current == 3 || current == 4, "current at {current}");
    for index in 0..LANGUAGE_INFO_ROWS {
      assert!(!app.row_selectable(index));
    }
    assert!(app.row_selectable(LANGUAGE_INFO_ROWS));
    assert!(app.row_selectable(LANGUAGE_INFO_ROWS + 1));
    assert!(!app.row_selectable(LANGUAGE_INFO_ROWS + 2));
    assert_eq!(app.navigation.current().selected, current);
  }

  #[test]
  fn language_selection_maps_to_languages() {
    assert_eq!(language_from_selected(LANGUAGE_INFO_ROWS), Lang::En);
    assert_eq!(language_from_selected(LANGUAGE_INFO_ROWS + 1), Lang::Pt);
    assert_eq!(language_from_selected(0), Lang::En);
  }

  #[test]
  fn fonts_dashboard_uses_icons_and_status() {
    let app = App::with_context(Page::Fonts, Lang::En, Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), 11, "7 font targets + 4 settings");
    assert!(rows[0].label.contains("Taskbar"), "{}", rows[0].label);
    assert!(rows[0].label.contains("🖥"), "{}", rows[0].label);
    assert!(rows[0].detail.is_some(), "font target should have detail");
    assert!(rows[7].label.contains("Antialiasing"), "{}", rows[7].label);
    assert!(rows[7].label.contains("✨"), "{}", rows[7].label);
    for (i, row) in rows.iter().enumerate() {
      assert!(row.detail.is_some(), "row {i} should have detail");
    }
  }

  #[test]
  fn fonts_dashboard_icons_are_distinct() {
    let mut seen = std::collections::HashSet::new();
    for target in FontTarget::ALL {
      let icon = font_target_icon(target);
      assert!(seen.insert(icon), "duplicate icon {icon} for {target:?}");
    }
    seen.clear();
    for setting in SettingKind::ALL {
      let icon = setting_icon(setting);
      assert!(seen.insert(icon), "duplicate icon {icon} for {setting:?}");
    }
  }

  #[test]
  fn default_apps_dashboard_uses_icons_and_status() {
    let app = App::with_context(Page::DefaultApps, Lang::En, Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), Category::ORDER.len());
    assert!(rows[0].label.contains("Terminal"), "{}", rows[0].label);
    assert!(rows[0].label.contains("🖥"), "{}", rows[0].label);
    assert!(rows[0].detail.is_some(), "category should have detail");
    assert!(rows[2].label.contains("Editor"), "{}", rows[2].label);
    assert!(rows[4].label.contains("Browser"), "{}", rows[4].label);
    for (i, row) in rows.iter().enumerate() {
      assert!(row.detail.is_some(), "row {i} should have detail");
    }
  }

  #[test]
  fn default_apps_dashboard_icons_are_distinct() {
    let mut seen = std::collections::HashSet::new();
    for category in Category::ORDER {
      let icon = category_icon(category);
      assert!(seen.insert(icon), "duplicate icon {icon} for {category:?}");
    }
  }

  #[test]
  fn system_dashboard_uses_icons_and_live_state() {
    let app = App::with_context(Page::System, Lang::En, Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].label.contains("Hostname"), "{}", rows[0].label);
    assert!(rows[0].label.contains("🖥"), "{}", rows[0].label);
    assert!(rows[0].detail.is_some(), "hostname should have detail");
    assert_eq!(rows[1].label, "👤 Users");
    assert_eq!(rows[1].detail.as_deref(), Some("N/A"));
    assert_eq!(rows[2].label, "👥 Groups");
    assert_eq!(rows[2].detail.as_deref(), Some("N/A"));
    for (index, row) in rows.iter().enumerate() {
      assert!(row.detail.is_some(), "row {index} should have detail");
    }
  }

  #[test]
  fn hostname_page_row_keeps_current_value_while_editing() {
    let mut app = App::with_context(Page::Hostname, Lang::En, Theme::load());
    app.hostname = "current-machine".into();
    app.hostname_editing = true;
    app.hostname_input = "draft-name".into();
    let rows = app.rows();
    assert_eq!(rows.len(), 1);
    assert!(
      rows[0].label.contains("Current hostname"),
      "{}",
      rows[0].label
    );
    assert_eq!(rows[0].detail.as_deref(), Some("current-machine"));
  }

  #[test]
  fn system_dashboard_counts_match_loaded_accounts() {
    let mut app = App::with_context(Page::System, Lang::En, Theme::load());
    app.admin.accounts = serde_json::json!({
      "actor_uid":1000,
      "shells":["/bin/bash", "/bin/zsh"],
      "groups":["users", "wheel"],
      "group_details":[
        {"name":"users","gid":1000,"members":[]},
        {"name":"wheel","gid":998,"members":["alice"]}
      ],
      "users":[
        {"user":"root","uid":0,"gid":0,"name":"root","shell":"/bin/bash","primary_group":"root","groups":[]},
        {"user":"alice","uid":1000,"gid":1000,"name":"Alice","shell":"/bin/bash","primary_group":"users","groups":["wheel"]}
      ]
    });
    let rows = app.rows();
    assert_eq!(rows[1].detail.as_deref(), Some("1"));
    assert_eq!(rows[2].detail.as_deref(), Some("1"));
    assert!(app.row_selectable(0));
    assert!(app.row_selectable(1));
    assert!(app.row_selectable(2));
  }

  #[test]
  fn task_window_scrolls_and_closes() {
    let mut app = App::with_context(Page::Main, Lang::En, Theme::load());
    let live = LiveProcess::new();
    for _ in 0..40 {
      live.push_line("line");
    }
    app.task_live = Some(live.clone());
    app.task_open = true;
    app.task_scroll = 0;
    let send = |app: &mut App, key: crossterm::event::KeyCode| {
      crate::event::handle(
        app,
        crossterm::event::Event::Key(crossterm::event::KeyEvent::from(key)),
      )
    };
    send(&mut app, crossterm::event::KeyCode::Down);
    assert_eq!(app.task_scroll, 1);
    send(&mut app, crossterm::event::KeyCode::Up);
    assert_eq!(app.task_scroll, 0);
    send(&mut app, crossterm::event::KeyCode::PageDown);
    assert_eq!(app.task_scroll, 10);
    send(&mut app, crossterm::event::KeyCode::Home);
    assert_eq!(app.task_scroll, 0);
    send(&mut app, crossterm::event::KeyCode::End);
    assert_eq!(
      app.task_scroll as usize,
      40usize.saturating_sub(TASK_CONTENT_HEIGHT)
    );
    send(&mut app, crossterm::event::KeyCode::Esc);
    assert!(!app.task_open);
    assert_eq!(app.task_scroll, 0);
  }

  #[test]
  fn task_bottom_offset_is_zero_for_short_output() {
    let mut app = App::with_context(Page::Main, Lang::En, Theme::load());
    let live = LiveProcess::new();
    live.push_line("done.");
    app.task_live = Some(live);
    app.task_open = true;
    crate::event::handle(
      &mut app,
      crossterm::event::Event::Key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::End,
      )),
    );
    assert_eq!(app.task_scroll, 0);
    assert_eq!(task_bottom_offset("done."), 0);
  }

  #[test]
  fn empty_poll_returns_false() {
    let mut app = App::with_context(Page::Main, Lang::En, Theme::load());
    assert!(!app.poll());
  }
}
