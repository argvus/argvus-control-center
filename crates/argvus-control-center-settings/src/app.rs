//! Implements application state and main-flow coordination in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use argvus_control_center_apps::catalog::Category;

use crate::config::apps::AppsBackend;
use crate::config::fonts::{FontSettings, FontTarget, SettingKind};
use crate::i18n::{Lang, na, tr};
use crate::navigation::{Navigation, Page};
use crate::system::fonts::{self, FontEntry};
use crate::system::keybindings;
use crate::system::{host, input, keyboard, locale, time};
use crate::theme::Theme;
use argvus_control_center_core::config::AppConfig;
use argvus_control_center_core::{
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedRequest, SystemSettingsOperation},
  process::{LiveProcess, ProcessRequest, ProcessRunner, SystemProcessRunner},
};

/// Defines the constant `MIN_WIDTH`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub const MIN_WIDTH: u16 = 60;
/// Defines the constant `MIN_HEIGHT`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub const MIN_HEIGHT: u16 = 15;

/// Defines the constant `LANGUAGE_INFO_ROWS`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const LANGUAGE_INFO_ROWS: usize = 3;
/// Defines the constant `REGIONAL_LOCALE_INFO_ROWS`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const REGIONAL_LOCALE_INFO_ROWS: usize = 2;

#[derive(Debug, Clone, Copy)]
/// Defines `StatusKind`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub enum StatusKind {
  Success,
  Error,
}

/// Represents `Status`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Status {
  pub text: String,
  pub kind: StatusKind,
  created: Instant,
}

#[derive(Debug, Clone)]
/// Represents `Row`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Row {
  pub label: String,
  pub detail: Option<String>,
  pub current: bool,
}

#[derive(Debug, Clone)]
/// Defines `PendingAction`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub enum PendingAction {
  Administration(String),
  ResetApps,
  ResetApp(Category),
  ResetFonts,
  ResetFont(FontTarget),
  ResetFontSetting(SettingKind),
  ResetKeybindings,
  ApplySystemLocales,
  SetNtp(bool),
}

#[derive(Debug, Clone, Copy)]
/// Defines `RatbagRowAction`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
enum RatbagRowAction {
  Device,
  Profile,
  Dpi,
  ReportRate,
}

/// Represents `App`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
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
  input: input::InputSettings,
  keybindings: Vec<keybindings::Binding>,
  keybinding_capturing: bool,
  keybinding_editor_modifiers: [bool; 4],
  keybinding_editor_key: String,
  keybinding_editor_field: usize,
  keybinding_detected: bool,
  keybinding_super_required: bool,
  keybinding_conflict: Option<(String, String, Vec<String>)>,
  keybinding_edit_id: Option<String>,
  keybinding_reload_deadline: Option<Instant>,
  input_loading: bool,
  input_load_job: Option<JobHandle<input::InputSettings>>,
  input_device_job: Option<JobHandle<(input::Devices, Vec<crate::system::ratbag::Device>)>>,
  input_last_device_refresh: Instant,
  input_pending: Option<(usize, i8, Instant)>,
  input_toggle_pending: Option<usize>,
  input_job: Option<JobHandle<input::InputSettings>>,
  ratbag_device: usize,
  ratbag_pending_path: Option<String>,
  ratbag_job: Option<JobHandle<Vec<crate::system::ratbag::Device>>>,
  hostname: String,
  jobs: JobManager,
  dnd_enabled: bool,
  dnd_job: Option<JobHandle<(bool, bool)>>,
  dnd_last_refresh: Instant,
  task_job: Option<JobHandle<Result<String, String>>>,
  pub task_live: Option<LiveProcess>,
  pub task_open: bool,
  pub task_scroll: u16,
  pub task_follow: bool,
}

impl App {
  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn new(initial: Page) -> Self {
    let lang = Lang::detect();
    Self::with_context(initial, lang, Theme::load())
  }

  /// Constructs `with_context` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
      input: input::InputSettings::default(),
      keybindings: keybindings::load(),
      keybinding_capturing: false,
      keybinding_editor_modifiers: [false; 4],
      keybinding_editor_key: String::new(),
      keybinding_editor_field: 0,
      keybinding_detected: false,
      keybinding_super_required: false,
      keybinding_conflict: None,
      keybinding_edit_id: None,
      keybinding_reload_deadline: None,
      input_loading: true,
      input_load_job: None,
      input_device_job: None,
      input_last_device_refresh: Instant::now(),
      input_pending: None,
      input_toggle_pending: None,
      input_job: None,
      ratbag_device: 0,
      ratbag_pending_path: None,
      ratbag_job: None,
      hostname,
      jobs: JobManager::default(),
      dnd_enabled: false,
      dnd_job: None,
      dnd_last_refresh: Instant::now() - Duration::from_secs(2),
      task_job: None,
      task_live: None,
      task_open: false,
      task_scroll: 0,
      task_follow: false,
    };
    app.input_load_job = Some(app.jobs.spawn(|_| Ok(input::load())));
    if initial != Page::Main {
      app.select_current();
    }
    app
  }

  /// Executes the `page` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn page(&self) -> Page {
    self.navigation.current().page
  }

  /// Executes the `rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn rows(&self) -> Vec<Row> {
    if crate::administration::is_page(self.page()) {
      return self
        .admin
        .rows_filtered(self.page(), self.lang, &self.search);
    }
    match self.page() {
      Page::Main => vec![
        Row::plain(tr(self.lang, "control_center.default_apps")),
        Row::plain(tr(self.lang, "control_center.fonts")),
        Row::plain(tr(self.lang, "control_center.locale_region")),
        Row::plain(tr(self.lang, "control_center.system")),
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
            AppConfig::icon(argvus_tui::icons::NETWORK),
            tr(self.lang, "control_center.time_zone")
          ),
          detail: Some(non_empty(&self.datetime.time_zone)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::HISTORY),
            tr(self.lang, "control_center.date_time")
          ),
          detail: Some(non_empty(&self.datetime.local_time)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::NETWORK),
            tr(self.lang, "control_center.regional_locale")
          ),
          detail: Some(locale::current_lang()),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::APPS),
            tr(self.lang, "control_center.system_locales")
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
            AppConfig::icon(argvus_tui::icons::KEYBOARD),
            tr(self.lang, "control_center.keyboard")
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
            AppConfig::icon(argvus_tui::icons::HISTORY),
            tr(self.lang, "control_center.local_date_time")
          ),
          detail: Some(self.datetime.local_time.clone()),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::NETWORK),
            tr(self.lang, "control_center.time_zone")
          ),
          detail: Some(self.datetime.time_zone.clone()),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::SATELLITE),
            tr(self.lang, "control_center.automatic_date_time_ntp")
          ),
          detail: Some(enabled_label(self.lang, self.datetime.ntp.unwrap_or(false)).to_string()),
          current: false,
        },
        Row {
          label: format!("{} RTC", AppConfig::icon(argvus_tui::icons::BATTERY)),
          detail: Some(
            if self.datetime.rtc_local.unwrap_or(false) {
              tr(self.lang, "control_center.local")
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
            label: tr(self.lang, "control_center.current_locale").to_string(),
            detail: Some(non_empty(&current)),
            current: false,
          },
          Row {
            label: tr(self.lang, "control_center.encoding").to_string(),
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
            AppConfig::icon(argvus_tui::icons::KEYBOARD),
            tr(self.lang, "control_center.layout")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_layout)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::FONTS),
            tr(self.lang, "control_center.variant")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_variant)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::MOUSE),
            tr(self.lang, "control_center.model")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_model)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::SETTINGS),
            tr(self.lang, "control_center.options")
          ),
          detail: Some(non_empty(&self.keyboard_info.x11_options)),
          current: false,
        },
        Row {
          label: format!(
            "{} {}",
            AppConfig::icon(argvus_tui::icons::MONITOR),
            tr(self.lang, "control_center.console_keymap")
          ),
          detail: Some(non_empty(&self.keyboard_info.console_keymap)),
          current: false,
        },
        Row {
          label: format!(
            "{} Hyprland XKB",
            AppConfig::icon(argvus_tui::icons::NETWORK)
          ),
          detail: Some(format!(
            "{} {} {}",
            non_empty(&self.keyboard_info.hypr_layout),
            non_empty(&self.keyboard_info.hypr_variant),
            non_empty(&self.keyboard_info.hypr_options)
          )),
          current: false,
        },
      ],
      Page::Keybindings => self.keybinding_rows(),
      Page::KeybindingEdit => self.keybinding_edit_rows(),
      Page::KeybindingCapture => self.keybinding_capture_rows(),
      Page::MouseTouchpad => self.input_rows(),
      Page::KeyboardLayout => self
        .keyboard_layouts
        .iter()
        .filter(|layout| search_matches(&self.search, &[&layout.code, &layout.description]))
        .map(|layout| Row {
          label: format!(
            "[{}] {}",
            if self
              .keyboard_info
              .hypr_layout
              .split(',')
              .any(|selected| selected.trim() == layout.code)
            {
              "✓"
            } else {
              " "
            },
            layout.code
          ),
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
            tr(self.lang, "control_center.default").to_string()
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
        label: tr(self.lang, "control_center.current_hostname").to_string(),
        detail: Some(self.hostname.clone()),
        current: false,
      }],
    }
  }

  /// Executes the `input_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn input_rows(&self) -> Vec<Row> {
    if self.input_loading {
      let loading = tr(self.lang, "control_center.input_loading").to_string();
      return [
        tr(self.lang, "control_center.input_mouse").to_string(),
        tr(self.lang, "control_center.input_pointer_speed").to_string(),
        tr(self.lang, "control_center.input_acceleration").to_string(),
        tr(self.lang, "control_center.input_natural_scrolling").to_string(),
        tr(self.lang, "control_center.input_scroll_speed").to_string(),
        tr(self.lang, "control_center.input_left_handed").to_string(),
        tr(self.lang, "control_center.input_touchpad").to_string(),
      ]
      .into_iter()
      .enumerate()
      .map(|(index, label)| Row {
        label: label.to_string(),
        detail: (index != 0).then(|| loading.clone()),
        current: false,
      })
      .collect();
    }
    let m = &self.input.mouse;
    let t = &self.input.touchpad;
    let mut rows = vec![
      Row::plain(tr(self.lang, "control_center.input_mouse")),
      Row {
        label: tr(self.lang, "control_center.input_pointer_speed").into(),
        detail: Some(input::speed_display(m.sensitivity)),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.input_acceleration").into(),
        detail: Some(tr(self.lang, input::accel_label(&m.accel_profile)).into()),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.input_natural_scrolling").into(),
        detail: Some(enabled_label(self.lang, m.natural_scroll).into()),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.input_scroll_speed").into(),
        detail: Some(input::factor_display(m.scroll_factor)),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.input_left_handed").into(),
        detail: Some(enabled_label(self.lang, m.left_handed).into()),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.input_touchpad").into(),
        detail: Some(
          if self.input.devices.touchpad {
            tr(self.lang, "control_center.available")
          } else {
            tr(self.lang, "control_center.not_available")
          }
          .into(),
        ),
        current: false,
      },
    ];
    if self.input.devices.touchpad {
      rows.extend([
        Row {
          label: tr(self.lang, "control_center.input_touchpad_natural_scrolling").into(),
          detail: Some(enabled_label(self.lang, t.natural_scroll).into()),
          current: false,
        },
        Row {
          label: tr(self.lang, "control_center.input_tap_to_click").into(),
          detail: Some(enabled_label(self.lang, t.tap_to_click).into()),
          current: false,
        },
        Row {
          label: tr(self.lang, "control_center.input_tap_and_drag").into(),
          detail: Some(enabled_label(self.lang, t.tap_and_drag).into()),
          current: false,
        },
        Row {
          label: tr(self.lang, "control_center.input_two_finger_right_click").into(),
          detail: Some(enabled_label(self.lang, t.two_finger_right_click).into()),
          current: false,
        },
        Row {
          label: tr(self.lang, "control_center.input_disable_while_typing").into(),
          detail: Some(enabled_label(self.lang, t.disable_while_typing).into()),
          current: false,
        },
      ]);
    }
    if !self.input.ratbag.is_empty() {
      let device = self
        .input
        .ratbag
        .get(self.ratbag_device)
        .or_else(|| self.input.ratbag.first())
        .expect("ratbag device list is not empty");
      rows.push(Row {
        label: tr(self.lang, "control_center.input_hardware_mouse").into(),
        detail: (self.input.ratbag.len() == 1).then(|| {
          if device.name.is_empty() {
            tr(self.lang, "control_center.input_hardware_device_unknown").to_string()
          } else {
            device.name.clone()
          }
        }),
        current: false,
      });
      if self.input.ratbag.len() > 1 {
        rows.push(Row {
          label: tr(self.lang, "control_center.input_hardware_device").into(),
          detail: Some(if device.name.is_empty() {
            tr(self.lang, "control_center.input_hardware_device_unknown").to_string()
          } else {
            device.name.clone()
          }),
          current: false,
        });
      }
      if let Some(profile) = device.active_profile() {
        let profile_name = if profile.name.is_empty() {
          self.lang.tr_args(
            "control_center.input_hardware_profile_number",
            [("number", profile.index.to_string())],
          )
        } else {
          profile.name.clone()
        };
        rows.push(Row {
          label: tr(self.lang, "control_center.input_hardware_profile").into(),
          detail: Some(profile_name),
          current: false,
        });
        if let Some(resolution) = profile.active_resolution() {
          let dpi = if resolution.dpi_x == resolution.dpi_y {
            resolution.dpi_x.to_string()
          } else {
            format!("{} × {}", resolution.dpi_x, resolution.dpi_y)
          };
          if profile.supports_dpi() {
            rows.push(Row {
              label: tr(self.lang, "control_center.input_hardware_dpi").into(),
              detail: Some(
                self
                  .lang
                  .tr_args("control_center.input_hardware_dpi_unit", [("value", dpi)]),
              ),
              current: false,
            });
          }
        }
        if profile.report_rate.is_some() && profile.supports_report_rate() {
          rows.push(Row {
            label: tr(self.lang, "control_center.input_hardware_polling_rate").into(),
            detail: Some(self.lang.tr_args(
              "control_center.input_hardware_polling_unit",
              [("value", profile.report_rate.unwrap_or_default().to_string())],
            )),
            current: false,
          });
        }
      }
    }
    rows
  }

  /// Executes the `ratbag_row_action` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn ratbag_row_action(&self, index: usize) -> Option<RatbagRowAction> {
    if self.input.ratbag.is_empty() {
      return None;
    }
    let device = self
      .input
      .ratbag
      .get(self.ratbag_device)
      .or_else(|| self.input.ratbag.first())?;
    let mut row = 7 + usize::from(self.input.devices.touchpad) * 5;
    if self.input.ratbag.len() > 1 {
      if index == row + 1 {
        return Some(RatbagRowAction::Device);
      }
      row += 1;
    }
    let profile = device.active_profile()?;
    if index == row + 1 {
      return Some(RatbagRowAction::Profile);
    }
    row += 1;
    if profile.supports_dpi() {
      if index == row + 1 {
        return Some(RatbagRowAction::Dpi);
      }
      row += 1;
    }
    if !profile.report_rates.is_empty() && index == row + 1 {
      return Some(RatbagRowAction::ReportRate);
    }
    None
  }

  /// Executes the `system_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
        label: format!("{} Hostname", AppConfig::icon(argvus_tui::icons::MONITOR)),
        detail: Some(hostname),
        current: false,
      },
      Row {
        label: format!(
          "{} {}",
          AppConfig::icon(argvus_tui::icons::USER),
          tr(self.lang, "control_center.users")
        ),
        detail: Some(users),
        current: false,
      },
      Row {
        label: format!(
          "{} {}",
          AppConfig::icon(argvus_tui::icons::USERS),
          tr(self.lang, "control_center.groups")
        ),
        detail: Some(groups),
        current: false,
      },
      Row {
        label: format!(
          "{} {}",
          AppConfig::icon(if self.dnd_enabled {
            argvus_tui::icons::BELL_OFF
          } else {
            argvus_tui::icons::BELL
          }),
          tr(self.lang, "control_center.do_not_disturb")
        ),
        detail: Some(if self.dnd_enabled {
          tr(self.lang, "control_center.enabled").to_string()
        } else {
          tr(self.lang, "control_center.disabled").to_string()
        }),
        current: false,
      },
    ]
  }

  /// Executes the `language_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn language_rows(&self) -> Vec<Row> {
    let current_lang = locale::current_lang();
    let current_language = if self.lang.locale() == "pt-BR" {
      tr(self.lang, "control_center.language_portuguese_brazil")
    } else {
      tr(self.lang, "control_center.language_english_us")
    };
    let in_use = tr(self.lang, "control_center.in_use");
    let mut rows = vec![
      Row {
        label: tr(self.lang, "control_center.current_language").to_string(),
        detail: Some(current_language.to_string()),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.regional_locale_90adc4").to_string(),
        detail: Some(non_empty(&current_lang)),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.encoding").to_string(),
        detail: Some(non_empty(&locale::encoding_from_locale(&current_lang))),
        current: false,
      },
    ];
    for (lang, flag, name_key, detail_key) in [
      (
        Lang::for_locale("en-US"),
        "🇺🇸",
        "control_center.language_english",
        "control_center.language_english_us",
      ),
      (
        Lang::for_locale("pt-BR"),
        "🇧🇷",
        "control_center.language_portuguese",
        "control_center.language_portuguese_brazil",
      ),
    ] {
      let badge = if self.lang == lang {
        format!(" · {in_use}")
      } else {
        String::new()
      };
      rows.push(Row {
        label: format!("{}  {}", AppConfig::icon(flag), tr(self.lang, name_key)),
        detail: Some(format!("{}{badge}", tr(self.lang, detail_key))),
        current: self.lang == lang,
      });
    }
    rows
  }

  /// Executes the `breadcrumb` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "control_center.argvus_control_center");
    match self.page() {
      Page::Main => root.to_string(),
      Page::DefaultApps => format!("{root} > {}", tr(self.lang, "control_center.default_apps")),
      Page::AppSelector(category) => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.default_apps"),
        category_label(self.lang, category)
      ),
      Page::Fonts => format!("{root} > {}", tr(self.lang, "control_center.fonts")),
      Page::FontSelector(target) => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.fonts"),
        font_target_label(self.lang, target)
      ),
      Page::SettingSelector(setting) => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.fonts"),
        setting_label(self.lang, setting)
      ),
      Page::LocaleRegion => format!("{root} > {}", tr(self.lang, "control_center.locale_region")),
      Page::MouseTouchpad => format!(
        "{root} > {}",
        tr(self.lang, "control_center.mouse_touchpad")
      ),
      Page::TimeZone => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.time_zone")
      ),
      Page::DateTime => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.date_time")
      ),
      Page::RegionalLocale => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.regional_locale")
      ),
      Page::SystemLocales => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.system_locales")
      ),
      Page::Keyboard => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.keyboard")
      ),
      Page::Keybindings => format!(
        "{root} > {}",
        tr(self.lang, "control_center.keyboard_shortcuts")
      ),
      Page::KeybindingEdit => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.keyboard_shortcuts"),
        tr(self.lang, "control_center.edit_shortcut")
      ),
      Page::KeybindingCapture => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.keyboard_shortcuts"),
        tr(self.lang, "control_center.change_shortcut")
      ),
      Page::KeyboardLayout => format!(
        "{root} > {} > {} > Layout",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.keyboard")
      ),
      Page::KeyboardVariant => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.keyboard"),
        tr(self.lang, "control_center.variant")
      ),
      Page::ConsoleKeymap => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "control_center.locale_region"),
        tr(self.lang, "control_center.keyboard"),
        tr(self.lang, "control_center.console_keymap")
      ),
      Page::Language => format!("{root} > {}", tr(self.lang, "control_center.language")),
      Page::System => format!("{root} > {}", tr(self.lang, "control_center.system")),
      Page::Firewall => format!(
        "{root} > {} > Firewall",
        tr(self.lang, "control_center.network")
      ),
      Page::Users => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.users")
      ),
      Page::UserList => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.users"),
        tr(self.lang, "control_center.list")
      ),
      Page::SystemUsers => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.users"),
        tr(self.lang, "control_center.system_accounts")
      ),
      Page::User
      | Page::CreateUser
      | Page::UserGroups
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.users")
      ),
      Page::Groups => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.groups")
      ),
      Page::GroupList => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.groups"),
        tr(self.lang, "control_center.list")
      ),
      Page::SystemGroups => format!(
        "{root} > {} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.groups"),
        tr(self.lang, "control_center.system_accounts")
      ),
      Page::Group | Page::GroupMembers | Page::CreateGroup => format!(
        "{root} > {} > {}",
        tr(self.lang, "control_center.system"),
        tr(self.lang, "control_center.groups")
      ),
      Page::Hostname => format!(
        "{root} > {} > Hostname",
        tr(self.lang, "control_center.system")
      ),
    }
  }

  /// Executes the `footer` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn footer(&self) -> &'static str {
    if self.confirm.is_some() {
      return tr(self.lang, "control_center.enter_confirm_esc_cancel");
    }
    if self.searching {
      return tr(
        self.lang,
        "control_center.type_to_search_enter_apply_esc_cancel",
      );
    }
    match self.page() {
      Page::Main => tr(self.lang, "control_center.navigate_enter_open_help_q_quit"),
      Page::DefaultApps => tr(
        self.lang,
        "control_center.navigate_enter_open_tab_actions_esc_back_help",
      ),
      Page::Fonts => tr(
        self.lang,
        "control_center.navigate_enter_open_tab_actions_esc_back_help",
      ),
      Page::AppSelector(_) => tr(
        self.lang,
        "control_center.navigate_tab_actions_move_enter_activate_search_esc_back_help",
      ),
      Page::FontSelector(_) => tr(
        self.lang,
        "control_center.navigate_enter_apply_size_search_tab_actions_esc_back_help",
      ),
      Page::SystemLocales => tr(
        self.lang,
        "control_center.navigate_space_toggle_search_enter_apply_esc_back",
      ),
      Page::Keybindings => tr(self.lang, "control_center.keybindings_help"),
      Page::KeybindingEdit => tr(self.lang, "control_center.keybindings_editor_actions"),
      Page::KeybindingCapture => tr(self.lang, "control_center.keybindings_capture_help"),
      Page::TimeZone
      | Page::RegionalLocale
      | Page::KeyboardLayout
      | Page::KeyboardVariant
      | Page::ConsoleKeymap => tr(
        self.lang,
        "control_center.navigate_enter_apply_search_esc_back_help",
      ),
      Page::Hostname => tr(self.lang, "control_center.enter_edit_esc_back_help"),
      Page::Language => tr(
        self.lang,
        "control_center.navigate_enter_apply_esc_back_help",
      ),
      Page::System => tr(
        self.lang,
        "control_center.navigate_enter_open_r_refresh_esc_back_help",
      ),
      Page::SettingSelector(_) => tr(
        self.lang,
        "control_center.navigate_tab_actions_move_enter_activate_esc_back_help",
      ),
      Page::UserList | Page::SystemUsers | Page::GroupList | Page::SystemGroups => tr(
        self.lang,
        "control_center.navigate_search_enter_open_esc_back_help",
      ),
      Page::User | Page::CreateUser | Page::CreateGroup | Page::Group | Page::Firewall => tr(
        self.lang,
        "control_center.fields_tab_switch_actions_enter_activate_esc_back_help",
      ),
      _ => tr(
        self.lang,
        "control_center.navigate_enter_open_esc_back_help",
      ),
    }
  }

  /// Executes the `move_selection` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Processes `on_buttons` in this module's event flow. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn on_buttons(&self) -> bool {
    let buttons = self.page_buttons();
    let rows = self.rows().len();
    !buttons.is_empty() && self.navigation.current().selected >= rows
  }

  /// Executes the `row_selectable` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn row_selectable(&self, index: usize) -> bool {
    if self.page() == Page::Keybindings {
      let rows = self.rows();
      if index < rows.len() {
        return index != 0
          && rows[index].detail.is_some()
          && !self.is_keybinding_column_header(&rows[index]);
      }
      return self.reset_page() && index < rows.len() + self.page_buttons().len();
    }
    if self.page() == Page::KeybindingEdit {
      return (2..=4).contains(&index);
    }
    if self.page() == Page::KeybindingCapture {
      return (2..=3).contains(&index);
    }
    if self.page() == Page::DateTime {
      return (index == 0 && !self.datetime.ntp.unwrap_or(false)) || index == 2;
    }
    if self.page() == Page::Keyboard {
      return matches!(index, 0 | 1 | 4);
    }
    if self.page() == Page::MouseTouchpad {
      if self.input_loading {
        return false;
      }
      if !self.input.ratbag.is_empty() && index == 7 + usize::from(self.input.devices.touchpad) * 5
      {
        return false;
      }
      if let Some(action) = self.ratbag_row_action(index) {
        return match action {
          RatbagRowAction::Device => self.input.ratbag.len() > 1,
          RatbagRowAction::Profile => self
            .input
            .ratbag
            .get(self.ratbag_device)
            .or_else(|| self.input.ratbag.first())
            .is_some_and(|device| device.profiles.len() > 1),
          RatbagRowAction::Dpi => self
            .input
            .ratbag
            .get(self.ratbag_device)
            .or_else(|| self.input.ratbag.first())
            .and_then(crate::system::ratbag::Device::active_profile)
            .is_some_and(crate::system::ratbag::Profile::supports_dpi),
          RatbagRowAction::ReportRate => self
            .input
            .ratbag
            .get(self.ratbag_device)
            .or_else(|| self.input.ratbag.first())
            .and_then(crate::system::ratbag::Device::active_profile)
            .is_some_and(crate::system::ratbag::Profile::supports_report_rate),
        };
      }
      if !self.input.devices.mouse && matches!(index, 1..=5) {
        return false;
      }
      return !matches!(index, 0 | 6);
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

  /// Executes the `keybinding_row_is_accent` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn keybinding_row_is_accent(&self, row: &Row) -> bool {
    self.page() == Page::Keybindings
      && (row.label.ends_with('»') || self.is_keybinding_column_header(row))
  }

  /// Checks the condition represented by `is_keybinding_column_header` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn is_keybinding_column_header(&self, row: &Row) -> bool {
    self.page() == Page::Keybindings
      && row.label == tr(self.lang, "control_center.keybindings_column_shortcut")
      && row.detail.as_deref()
        == Some(tr(
          self.lang,
          "control_center.keybindings_column_description",
        ))
  }

  /// Executes the `normalize_selection` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `item_count` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `cycle_selection` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `move_button` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `admin_buttons` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn admin_buttons(&self, page: Page) -> Vec<crate::administration::Button> {
    self.admin.buttons(page, self.lang)
  }

  /// Executes the `reset_page` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn reset_page(&self) -> bool {
    matches!(
      self.page(),
      Page::DefaultApps
        | Page::Fonts
        | Page::AppSelector(_)
        | Page::FontSelector(_)
        | Page::SettingSelector(_)
        | Page::Keybindings
    )
  }

  /// Executes the `page_buttons` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn page_buttons(&self) -> Vec<crate::administration::Button> {
    let page = self.page();
    if crate::administration::is_page(page) {
      self.admin.buttons(page, self.lang)
    } else if self.reset_page() {
      vec![crate::administration::Button::new(
        if page == Page::Keybindings {
          tr(self.lang, "control_center.restore_all_shortcuts")
        } else {
          tr(self.lang, "control_center.reset_defaults")
        },
        crate::administration::ButtonKind::Secondary,
      )]
    } else {
      Vec::new()
    }
  }

  /// Executes the `open_or_apply` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
            tr(self.lang, "control_center.local_date_time").into(),
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
      Page::MouseTouchpad => self.apply_input(selected),
      Page::Keybindings => self.open_keybinding_edit(),
      Page::KeybindingEdit => self.apply_keybinding_edit_selection(),
      Page::KeybindingCapture => self.apply_keybinding_capture_selection(),
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
        3 => self.toggle_dnd(),
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

  /// Executes the `reset_current` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn reset_current(&mut self) {
    match self.page() {
      Page::DefaultApps => self.open_confirm(PendingAction::ResetApps),
      Page::AppSelector(category) => self.open_confirm(PendingAction::ResetApp(category)),
      Page::Fonts => self.open_confirm(PendingAction::ResetFonts),
      Page::FontSelector(target) => self.open_confirm(PendingAction::ResetFont(target)),
      Page::SettingSelector(setting) => {
        self.open_confirm(PendingAction::ResetFontSetting(setting));
      }
      Page::Keybindings => {
        if self.on_buttons() {
          self.open_confirm(PendingAction::ResetKeybindings)
        } else {
          self.restore_keybinding()
        }
      }
      Page::KeybindingEdit => {
        self.restore_keybinding();
        self.navigation.back();
      }
      _ => {}
    }
  }

  /// Executes the `refresh_system` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn refresh_system(&mut self) {
    self.hostname = host::current();
    self.admin.load(false);
    self.success(tr(self.lang, "control_center.data_updated").to_string());
  }

  /// Executes the `refresh_dnd` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn refresh_dnd(&mut self) {
    if self.dnd_job.is_some() {
      return;
    }
    self.dnd_job = Some(self.jobs.spawn(|_| {
      Self::read_dnd_command(
        &ProcessRequest::new("argvus-notifications")
          .arg("dnd")
          .arg("status"),
      )
      .map(|enabled| (enabled, false))
    }));
    self.dnd_last_refresh = Instant::now();
  }

  /// Applies the `toggle_dnd` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn toggle_dnd(&mut self) {
    if self.dnd_job.is_some() {
      return;
    }
    self.dnd_job = Some(self.jobs.spawn(|_| {
      Self::read_dnd_command(
        &ProcessRequest::new("argvus-notifications")
          .arg("dnd")
          .arg("toggle"),
      )
      .map(|state| (state, true))
    }));
  }

  /// Retrieves data for `read_dnd_command` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn read_dnd_command(request: &ProcessRequest) -> Result<bool, String> {
    let output = SystemProcessRunner
      .run(&request.clone().timeout(Duration::from_secs(2)))
      .map_err(|error| error.to_string())?;
    if output.timed_out || output.status != Some(0) {
      let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
      return Err(if error.is_empty() {
        "notification backend failed".into()
      } else {
        error
      });
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
      "dnd=true" => Ok(true),
      "dnd=false" => Ok(false),
      value => Err(format!("invalid notification state: {value}")),
    }
  }

  /// Applies the `toggle_current` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn toggle_current(&mut self) {
    if crate::administration::is_page(self.page()) {
      self.admin_open();
      return;
    }
    if self.page() == Page::KeyboardLayout {
      self.toggle_keyboard_layout();
      return;
    }
    if self.page() == Page::MouseTouchpad {
      let selected = self.navigation.current().selected;
      self.apply_input(selected);
      return;
    }
    if self.page() == Page::Keybindings {
      self.toggle_keybinding(true);
      return;
    }
    if self.page() == Page::KeybindingEdit {
      self.apply_keybinding_edit_selection();
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

  /// Applies the `apply_input` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_input(&mut self, selected: usize) {
    if let Some(action) = self.ratbag_row_action(selected) {
      match action {
        RatbagRowAction::Device => self.cycle_ratbag_device(1),
        RatbagRowAction::Profile => self.queue_ratbag(crate::system::ratbag::Change::Profile(1)),
        RatbagRowAction::Dpi => self.queue_ratbag(crate::system::ratbag::Change::Dpi(1)),
        RatbagRowAction::ReportRate => {
          self.queue_ratbag(crate::system::ratbag::Change::ReportRate(1))
        }
      }
      return;
    }
    if matches!(selected, 3 | 5 | 7..=11) {
      self.input_toggle_pending = Some(selected);
      self.status = Some(Status {
        text: tr(self.lang, "control_center.input_applying").to_string(),
        kind: StatusKind::Success,
        created: Instant::now(),
      });
    } else {
      self.input_cycle(selected, 1);
    }
  }

  /// Executes the `input_cycle` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub(crate) fn input_cycle(&mut self, selected: usize, direction: i8) {
    if let Some(action) = self.ratbag_row_action(selected) {
      match action {
        RatbagRowAction::Device => self.cycle_ratbag_device(direction),
        RatbagRowAction::Profile => {
          self.queue_ratbag(crate::system::ratbag::Change::Profile(direction))
        }
        RatbagRowAction::Dpi => self.queue_ratbag(crate::system::ratbag::Change::Dpi(direction)),
        RatbagRowAction::ReportRate => {
          self.queue_ratbag(crate::system::ratbag::Change::ReportRate(direction))
        }
      }
      return;
    }
    let direction = direction.signum();
    let pending = self.input_pending.take();
    self.input_pending = Some(match pending {
      Some((pending_selected, pending_direction, _)) if pending_selected == selected => (
        selected,
        pending_direction.saturating_add(direction),
        Instant::now() + Duration::from_millis(100),
      ),
      _ => (
        selected,
        direction,
        Instant::now() + Duration::from_millis(100),
      ),
    });
    self.status = Some(Status {
      text: tr(self.lang, "control_center.input_applying").to_string(),
      kind: StatusKind::Success,
      created: Instant::now(),
    });
  }

  /// Executes the `input_success` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub(crate) fn input_success(&mut self) {
    self.success(tr(self.lang, "control_center.input_applied").to_string());
  }

  /// Executes the `input_failure` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub(crate) fn input_failure(&mut self, error: String) {
    eprintln!("argvus-control-center: input setting failed: {error}");
    self.fail(tr(self.lang, "control_center.input_apply_failed"));
  }

  /// Executes the `cycle_ratbag_device` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn cycle_ratbag_device(&mut self, direction: i8) {
    let Some(next) = crate::system::ratbag::next_device_index(
      self.ratbag_device,
      self.input.ratbag.len(),
      direction,
    ) else {
      return;
    };
    self.ratbag_device = next;
  }

  /// Executes the `queue_ratbag` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn queue_ratbag(&mut self, change: crate::system::ratbag::Change) {
    if self.ratbag_job.is_some() {
      return;
    }
    let Some(device) = self
      .input
      .ratbag
      .get(self.ratbag_device)
      .or_else(|| self.input.ratbag.first())
      .cloned()
    else {
      return;
    };
    self.status = Some(Status {
      text: tr(self.lang, "control_center.input_hardware_applying").to_string(),
      kind: StatusKind::Success,
      created: Instant::now(),
    });
    let device_path = device.path.clone();
    self.ratbag_job = Some(
      self
        .jobs
        .spawn(move |_| crate::system::ratbag::apply(&device, change)),
    );
    self.ratbag_pending_path = Some(device_path);
  }

  /// Applies the `toggle_keyboard_layout` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn toggle_keyboard_layout(&mut self) {
    let layouts: Vec<keyboard::Layout> = self
      .keyboard_layouts
      .iter()
      .filter(|layout| search_matches(&self.search, &[&layout.code, &layout.description]))
      .cloned()
      .collect();
    let Some(layout) = layouts.get(self.navigation.current().selected) else {
      return;
    };
    let mut selected: Vec<String> = self
      .keyboard_info
      .hypr_layout
      .split(',')
      .map(str::trim)
      .filter(|layout| !layout.is_empty())
      .map(str::to_string)
      .collect();
    if let Some(index) = selected.iter().position(|value| value == &layout.code) {
      if selected.len() == 1 {
        self.fail("at least one keyboard layout must remain selected");
        return;
      }
      selected.remove(index);
    } else {
      selected.push(layout.code.clone());
    }
    let default_layout = if selected
      .iter()
      .any(|value| value == &self.keyboard_info.x11_layout)
    {
      self.keyboard_info.x11_layout.clone()
    } else {
      selected.first().cloned().unwrap_or_default()
    };
    match keyboard::set_x11_layouts(&default_layout, &selected, &self.keyboard_layouts) {
      Ok(()) => {
        self.refresh_keyboard();
        self.success(tr(self.lang, "control_center.keyboard_layouts_updated").to_string());
      }
      Err(error) => self.fail(error),
    }
  }

  /// Executes the `confirm_accept` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
        Ok(()) => {
          self.success(tr(self.lang, "control_center.default_applications_restored").to_string())
        }
        Err(error) => self.fail(error),
      },
      PendingAction::ResetApp(category) => match self.apps.reset_default(category) {
        Ok(()) => self.success(format!(
          "{} → {}",
          category_label(self.lang, category),
          tr(self.lang, "control_center.argvus_default")
        )),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetFonts => match self.fonts.reset_all() {
        Ok(()) => self.success(tr(self.lang, "control_center.font_settings_restored").to_string()),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetFont(target) => match self.fonts.reset_font(target) {
        Ok(()) => self.success(format!(
          "{} → {}",
          font_target_label(self.lang, target),
          tr(self.lang, "control_center.argvus_default")
        )),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetFontSetting(setting) => match self.fonts.reset_setting(setting) {
        Ok(()) => self.success(format!(
          "{} → {}",
          setting_label(self.lang, setting),
          tr(self.lang, "control_center.argvus_default")
        )),
        Err(error) => self.fail(error),
      },
      PendingAction::ResetKeybindings => match keybindings::restore_all(&self.keybindings) {
        Ok(()) => {
          self.keybindings = keybindings::load();
          let _ = std::process::Command::new("argvus-sessionctl")
            .arg("reload")
            .status();
          self.success(tr(self.lang, "control_center.keybindings_restored_all").to_string());
        }
        Err(error) => self.fail(error),
      },
      PendingAction::ApplySystemLocales => self.apply_system_locales(),
      PendingAction::Administration(_) => self.admin.submit(),
      PendingAction::SetNtp(enabled) => match time::set_ntp(enabled) {
        Ok(()) => {
          self.refresh_time();
          self.success(tr(self.lang, "control_center.ntp_setting_applied").to_string());
        }
        Err(error) => self.fail(error),
      },
    }
  }

  /// Executes the `cancel_modal` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn cancel_modal(&mut self) {
    self.admin.cancel_pending();
    self.confirm = None;
    self.confirm_apply_selected = false;
  }

  /// Applies the `toggle_confirm_button` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn toggle_confirm_button(&mut self) {
    if self.confirm.is_some() {
      self.confirm_apply_selected = !self.confirm_apply_selected;
    }
  }

  /// Executes the `hostname_input` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
              tr(self.lang, "control_center.hostname_changed")
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

  /// Executes the `back` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `begin_search` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
        | Page::Keybindings
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

  /// Executes the `push_search` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn push_search(&mut self, character: char) {
    self.search.push(character);
    self.navigation.current_mut().selected = 0;
    self.navigation.current_mut().scroll = 0;
  }

  /// Executes the `pop_search` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn pop_search(&mut self) {
    self.search.pop();
    self.navigation.current_mut().selected = 0;
    self.navigation.current_mut().scroll = 0;
  }

  /// Executes the `adjust_size` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn adjust_size(&mut self, delta: i16) {
    if matches!(self.page(), Page::FontSelector(_)) && !self.searching {
      self.pending_size = (self.pending_size as i16 + delta).clamp(8, 32) as u16;
    }
  }

  /// Executes the `resize` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn resize(&mut self, width: u16, height: u16) {
    self.width = width;
    self.height = height;
  }

  /// Executes the `too_small` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn too_small(&self) -> bool {
    self.width < MIN_WIDTH || self.height < MIN_HEIGHT
  }

  /// Applies the `set_viewport` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn set_viewport(&mut self, viewport: usize) {
    self.viewport = viewport.max(1);
    let location = self.navigation.current_mut();
    location.scroll = location.scroll.min(
      location
        .selected
        .saturating_sub(self.viewport.saturating_sub(1)),
    );
    self.ensure_visible(self.rows().len());
  }

  /// Executes the `expire_status` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
          self.success(tr(self.lang, "control_center.data_updated").into());
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

  /// Executes the `poll` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if self
      .keybinding_reload_deadline
      .is_some_and(|deadline| deadline <= Instant::now())
    {
      self.keybinding_reload_deadline = None;
      let _ = std::process::Command::new("argvus-sessionctl")
        .arg("reload")
        .status();
      changed = true;
    }
    if self.dnd_job.is_none()
      && self.page() == Page::System
      && self.dnd_last_refresh.elapsed() >= Duration::from_secs(2)
    {
      self.refresh_dnd();
    }
    if let Some(job) = self.dnd_job.take() {
      match job.try_state() {
        JobState::Running => self.dnd_job = Some(job),
        JobState::Finished(Ok((enabled, action))) => {
          self.dnd_enabled = enabled;
          if action {
            self.success(if enabled {
              tr(self.lang, "control_center.dnd_enabled").to_string()
            } else {
              tr(self.lang, "control_center.dnd_disabled").to_string()
            });
          }
          changed = true;
        }
        JobState::Finished(Err(error)) => {
          self.fail(format!(
            "{}: {error}",
            tr(self.lang, "control_center.notification_state_error")
          ));
          changed = true;
        }
      }
    }
    if let Some(job) = self.input_load_job.take() {
      match job.try_state() {
        JobState::Running => self.input_load_job = Some(job),
        JobState::Finished(Ok(settings)) => {
          self.input = settings;
          self.input_loading = false;
          self.input_last_device_refresh = Instant::now();
          self.normalize_selection();
          changed = true;
        }
        JobState::Finished(Err(error)) => {
          self.input_loading = false;
          self.input_failure(error);
          changed = true;
        }
      }
    }
    if !self.input_loading
      && self.page() == Page::MouseTouchpad
      && self.input_device_job.is_none()
      && self.input_last_device_refresh.elapsed() >= Duration::from_secs(2)
    {
      self.input_last_device_refresh = Instant::now();
      self.input_device_job = Some(
        self
          .jobs
          .spawn(|_| Ok((input::detect_devices(), crate::system::ratbag::devices()))),
      );
      changed = true;
    }
    if let Some(job) = self.input_device_job.take() {
      match job.try_state() {
        JobState::Running => self.input_device_job = Some(job),
        JobState::Finished(Ok((devices, ratbag))) => {
          self.input.devices = devices;
          self.input.ratbag = ratbag;
          self.ratbag_device = self
            .ratbag_device
            .min(self.input.ratbag.len().saturating_sub(1));
          self.normalize_selection();
          changed = true;
        }
        JobState::Finished(Err(error)) => {
          eprintln!("argvus-control-center: input device refresh failed: {error}");
        }
      }
    }
    if self.input_job.is_none()
      && let Some(selected) = self.input_toggle_pending.take()
    {
      let mut settings = self.input.clone();
      self.input_job = Some(self.jobs.spawn(move |_| {
        settings.toggle(selected)?;
        Ok(settings)
      }));
      changed = true;
    }
    if self.input_job.is_none()
      && let Some((selected, direction, deadline)) = self.input_pending
      && deadline <= Instant::now()
    {
      self.input_pending = None;
      let mut settings = self.input.clone();
      self.input_job = Some(self.jobs.spawn(move |_| {
        settings.cycle_by(selected, direction)?;
        Ok(settings)
      }));
      changed = true;
    }
    if let Some(job) = self.input_job.take() {
      match job.try_state() {
        JobState::Running => self.input_job = Some(job),
        JobState::Finished(Ok(settings)) => {
          self.input = settings;
          self.input_success();
          changed = true;
        }
        JobState::Finished(Err(error)) => {
          self.input_failure(error);
          changed = true;
        }
      }
    }
    if let Some(job) = self.ratbag_job.take() {
      match job.try_state() {
        JobState::Running => self.ratbag_job = Some(job),
        JobState::Finished(Ok(devices)) => {
          let requested_path = self.ratbag_pending_path.take();
          self.input.ratbag = devices;
          if let Some(path) = requested_path
            && let Some(index) = self
              .input
              .ratbag
              .iter()
              .position(|device| device.path == path)
          {
            self.ratbag_device = index;
          }
          self.ratbag_device = self
            .ratbag_device
            .min(self.input.ratbag.len().saturating_sub(1));
          self.success(tr(self.lang, "control_center.input_applied").to_string());
          self.normalize_selection();
          changed = true;
        }
        JobState::Finished(Err(error)) => {
          self.ratbag_pending_path = None;
          eprintln!("argvus-control-center: ratbag setting failed: {error}");
          self.fail(tr(self.lang, "control_center.input_hardware_apply_failed"));
          changed = true;
        }
      }
    }
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
          self.success(tr(self.lang, "control_center.locales_generated_successfully").to_string());
          changed = true;
        }
        JobState::Finished(Ok(Err(error))) | JobState::Finished(Err(error)) => self.fail(error),
      }
    }
    changed
  }

  /// Executes the `open_system_page` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
          "control_center.locales_backend_is_implemented_only_for_arch_linux"
        )
      ));
      return;
    }
    self.navigation.push(page);
    self.select_current();
  }

  /// Executes the `open_confirm` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn open_confirm(&mut self, action: PendingAction) {
    self.confirm = Some(action);
    self.confirm_apply_selected = false;
  }

  /// Applies the `apply_app` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
          tr(self.lang, "control_center.default_application_changed")
        )),
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_font` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
          tr(self.lang, "control_center.font_applied"),
          font.display_name(),
          self.pending_size
        )),
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_font_setting` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_font_setting(&mut self, setting: SettingKind, selected: usize) {
    if let Some(row) = self.setting_options(setting).get(selected) {
      let value = row.detail.as_deref().unwrap_or(&row.label).to_string();
      match self.fonts.apply_setting(setting, &value) {
        Ok(()) => self.success(tr(self.lang, "control_center.setting_applied").to_string()),
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_timezone` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
            tr(self.lang, "control_center.time_zone_changed")
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_regional_locale` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
          tr(self.lang, "control_center.regional_locale_changed")
        )),
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_system_locales` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
    let applied = tr(self.lang, "control_center.applied").to_string();
    let operation = SystemSettingsOperation::new(SystemProcessRunner, executable);
    self.task_job = Some(self.jobs.spawn(move |_| {
      Ok(match operation.execute_live(&request, &live) {
        Ok(output) if output.status == Some(0) => {
          let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
          Ok(if stdout.is_empty() { applied } else { stdout })
        }
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(error) => Err(error.to_string()),
      })
    }));
  }

  /// Applies the `apply_keyboard_layout` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
            tr(self.lang, "control_center.keyboard_layout_changed"),
            layout.code
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_keyboard_variant` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
            tr(self.lang, "control_center.variant_changed"),
            variant.description
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_console_keymap` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
            tr(self.lang, "control_center.console_keymap_changed")
          ));
        }
        Err(error) => self.fail(error),
      }
    }
  }

  /// Applies the `apply_language` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_language(&mut self, selected: usize) {
    self.lang = language_from_selected(selected);
    let path = crate::config::paths::argvus_config_home().join("language");
    if let Err(error) = std::fs::create_dir_all(crate::config::paths::argvus_config_home()) {
      self.fail(error);
      return;
    }
    let value = if self.lang.locale() == "pt-BR" {
      "pt_BR"
    } else {
      "en_US"
    };
    if let Err(error) = std::fs::write(path, format!("{value}\n")) {
      self.fail(error);
    } else {
      self.success(tr(self.lang, "control_center.interface_language_applied").to_string());
    }
  }

  /// Executes the `refresh_time` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub(crate) fn refresh_time(&mut self) {
    self.datetime = time::datetime_info();
  }

  /// Executes the `refresh_keyboard` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn refresh_keyboard(&mut self) {
    self.keyboard_info = keyboard::info();
    self.keyboard_variants = keyboard::variants(&self.keyboard_info.x11_layout);
  }

  /// Executes the `select_current` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn select_current(&mut self) {
    if let Some(index) = self.rows().iter().position(|row| row.current) {
      self.navigation.current_mut().selected = index;
      self.ensure_visible(self.rows().len());
    } else {
      self.normalize_selection();
    }
  }

  /// Executes the `visible_keybindings` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn visible_keybindings(&self) -> Vec<&keybindings::Binding> {
    let mut bindings: Vec<_> = self
      .keybindings
      .iter()
      .filter(|binding| keybinding_is_cheatsheet_entry(binding))
      .filter(|binding| {
        search_matches(
          &self.search,
          &[
            &binding.id,
            &binding.category,
            &binding.keys,
            &keybindings::display_keys(&binding.keys),
            &binding.description_key,
            &keybinding_label(self.lang, binding),
            &keybinding_section(self.lang, binding),
          ],
        )
      })
      .collect();
    bindings.sort_by_key(|binding| {
      (
        keybinding_section_order(binding),
        keybinding_category_order(&binding.category),
        binding.id.as_str(),
      )
    });
    bindings
  }

  /// Executes the `selected_keybinding` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn selected_keybinding(&self) -> Option<&keybindings::Binding> {
    let selected = self.navigation.current().selected;
    let row_count = self
      .rows()
      .into_iter()
      .skip(1)
      .take(selected.saturating_sub(1))
      .filter(|row| row.detail.is_some() && !self.is_keybinding_column_header(row))
      .count();
    self.visible_keybindings().get(row_count).copied()
  }

  /// Executes the `keybinding_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn keybinding_rows(&self) -> Vec<Row> {
    let mut rows = vec![Row {
      label: format!(
        "{}: {}",
        tr(self.lang, "control_center.search_shortcuts"),
        self.search
      ),
      detail: Some(String::new()),
      current: false,
    }];
    let mut section = String::new();
    let mut header_added = false;
    for binding in self.visible_keybindings() {
      let binding_section = keybinding_section(self.lang, binding);
      if section != binding_section {
        section = binding_section.clone();
        rows.push(Row::plain(""));
        rows.push(Row {
          label: binding_section,
          detail: None,
          current: false,
        });
        rows.push(Row {
          label: tr(self.lang, "control_center.keybindings_column_shortcut").to_string(),
          detail: Some(tr(self.lang, "control_center.keybindings_column_description").to_string()),
          current: false,
        });
        rows.push(Row::plain(""));
        header_added = true;
      }
      rows.push(Row {
        label: if binding.enabled {
          keybindings::display_keys(&binding.keys)
        } else {
          tr(self.lang, "control_center.disabled").to_string()
        },
        detail: Some(keybinding_label(self.lang, binding)),
        current: false,
      });
    }
    if !header_added {
      rows.retain(|row| row.detail.is_some());
    }
    rows
  }

  /// Executes the `edited_binding` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn edited_binding(&self) -> Option<&keybindings::Binding> {
    self
      .keybinding_edit_id
      .as_deref()
      .and_then(|id| self.keybindings.iter().find(|b| b.id == id))
  }

  /// Executes the `open_keybinding_edit` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn open_keybinding_edit(&mut self) {
    let Some(id) = self.selected_keybinding().map(|binding| binding.id.clone()) else {
      return;
    };
    self.keybinding_edit_id = Some(id);
    self.navigation.push(Page::KeybindingEdit);
    self.navigation.current_mut().selected = 2;
    self.begin_keybinding_capture();
    self.keybinding_capturing = false;
    self.status = None;
  }

  /// Executes the `keybinding_edit_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn keybinding_edit_rows(&self) -> Vec<Row> {
    let Some(binding) = self.edited_binding() else {
      return Vec::new();
    };
    vec![
      Row {
        label: keybinding_label(self.lang, binding),
        detail: Some(keybinding_category(self.lang, &binding.category)),
        current: false,
      },
      Row {
        label: tr(self.lang, "control_center.keybindings_current_shortcut").into(),
        detail: Some(if binding.enabled {
          keybindings::display_keys(&binding.keys)
        } else {
          tr(self.lang, "control_center.disabled").into()
        }),
        current: false,
      },
      Row {
        label: format!("[ {} ]", tr(self.lang, "control_center.change_shortcut")),
        detail: None,
        current: false,
      },
      Row {
        label: format!("[ {} ]", tr(self.lang, "control_center.disable")),
        detail: None,
        current: false,
      },
      Row {
        label: format!("[ {} ]", tr(self.lang, "control_center.restore_default")),
        detail: None,
        current: false,
      },
    ]
  }

  /// Executes the `keybinding_capture_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn keybinding_capture_rows(&self) -> Vec<Row> {
    let Some(binding) = self.edited_binding() else {
      return Vec::new();
    };
    let label = if self.keybinding_detected {
      tr(self.lang, "control_center.keybindings_detected_shortcut")
    } else {
      tr(self.lang, "control_center.press_new_shortcut")
    };
    let shortcut = if self.keybinding_detected {
      self.captured_shortcut()
    } else {
      String::new()
    };
    vec![
      Row {
        label: keybinding_label(self.lang, binding),
        detail: None,
        current: false,
      },
      Row {
        label: label.to_string(),
        detail: Some(shortcut),
        current: false,
      },
      Row {
        label: format!(
          "[ {} ]",
          if self.keybinding_detected {
            tr(self.lang, "control_center.apply")
          } else {
            tr(self.lang, "control_center.try_again")
          }
        ),
        detail: None,
        current: false,
      },
      Row {
        label: format!("[ {} ]", tr(self.lang, "control_center.cancel")),
        detail: None,
        current: false,
      },
    ]
  }

  #[allow(dead_code)]
  /// Executes the `keybinding_picker_rows` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn keybinding_picker_rows(&self) -> Vec<Row> {
    let mouse = self
      .edited_binding()
      .is_some_and(|b| matches!(b.input.as_str(), "mouse" | "mouse_button" | "mouse_wheel"));
    let options: Vec<(&str, &str)> = if mouse {
      vec![
        ("Mouse", "mouse:272"),
        ("Mouse", "mouse:273"),
        ("Mouse", "mouse:274"),
        ("Wheel", "mouse:275"),
        ("Wheel", "mouse:276"),
      ]
    } else {
      let mut v = vec![
        ("Common", "Return"),
        ("Common", "Space"),
        ("Common", "Tab"),
        ("Common", "Escape"),
        ("Common", "BackSpace"),
        ("Common", "Delete"),
        ("Navigation", "left"),
        ("Navigation", "right"),
        ("Navigation", "up"),
        ("Navigation", "down"),
        ("Navigation", "Home"),
        ("Navigation", "End"),
        ("Navigation", "Page_Up"),
        ("Navigation", "Page_Down"),
        ("Navigation", "Insert"),
        ("Symbols", "minus"),
        ("Symbols", "equal"),
        ("Symbols", "bracketleft"),
        ("Symbols", "bracketright"),
        ("Symbols", "backslash"),
        ("Symbols", "semicolon"),
        ("Symbols", "apostrophe"),
        ("Symbols", "comma"),
        ("Symbols", "period"),
        ("Symbols", "slash"),
        ("Symbols", "grave"),
      ];
      for n in 1..=24 {
        v.push(("Function", Box::leak(format!("F{n}").into_boxed_str())));
      }
      v
    };
    options
      .into_iter()
      .map(|(group, key)| Row {
        label: format!("{group}: {}", keybindings::display_key(key)),
        detail: None,
        current: false,
      })
      .collect()
  }

  /// Applies the `apply_keybinding_capture_selection` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_keybinding_capture_selection(&mut self) {
    match self.navigation.current().selected {
      2 if self.keybinding_detected => self.apply_keybinding_editor(),
      2 => self.keybinding_capturing = true,
      3 => {
        self.keybinding_capturing = false;
        self.navigation.back();
      }
      _ => {}
    }
  }

  /// Executes the `captured_shortcut` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn captured_shortcut(&self) -> String {
    let mut parts = Vec::new();
    for (enabled, modifier) in self
      .keybinding_editor_modifiers
      .into_iter()
      .zip(["CTRL", "ALT", "SHIFT", "SUPER"])
    {
      if enabled {
        parts.push(modifier);
      }
    }
    parts.push(self.keybinding_editor_key.as_str());
    keybindings::display_keys(&parts.join(" + "))
  }

  /// Applies the `apply_keybinding_edit_selection` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_keybinding_edit_selection(&mut self) {
    match self.navigation.current().selected {
      2 => {
        self.keybinding_capturing = true;
        self.keybinding_detected = false;
        self.navigation.push(Page::KeybindingCapture);
        self.navigation.current_mut().selected = 2;
      }
      3 => {
        self.keybinding_capturing = false;
        self.toggle_keybinding(false);
        self.navigation.back();
      }
      4 => {
        self.keybinding_capturing = false;
        self.restore_keybinding();
        self.navigation.back();
      }
      _ => {}
    }
  }

  #[allow(dead_code)]
  /// Executes the `select_keybinding_picker` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn select_keybinding_picker(&mut self) {
    let Some(binding) = self.edited_binding() else {
      return;
    };
    let mouse = matches!(
      binding.input.as_str(),
      "mouse" | "mouse_button" | "mouse_wheel"
    );
    let options = if mouse {
      vec![
        "mouse:272",
        "mouse:273",
        "mouse:274",
        "mouse:275",
        "mouse:276",
      ]
    } else {
      let mut v = vec![
        "Return",
        "Space",
        "Tab",
        "Escape",
        "BackSpace",
        "Delete",
        "left",
        "right",
        "up",
        "down",
        "Home",
        "End",
        "Page_Up",
        "Page_Down",
        "Insert",
        "minus",
        "equal",
        "bracketleft",
        "bracketright",
        "backslash",
        "semicolon",
        "apostrophe",
        "comma",
        "period",
        "slash",
        "grave",
      ];
      v.extend((1..=24).map(|n| Box::leak(format!("F{n}").into_boxed_str()) as &str));
      v
    };
    if let Some(key) = options.get(self.navigation.current().selected).copied() {
      self.keybinding_editor_key = key.into();
    }
    self.navigation.back();
  }

  /// Applies the `toggle_keybinding` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn toggle_keybinding(&mut self, enabled: bool) {
    let binding = self.edited_binding().or_else(|| self.selected_keybinding());
    let Some(binding) = binding else {
      return;
    };
    let id = binding.id.clone();
    let keys = binding.keys.clone();
    if let Err(error) = keybindings::save_override(
      &id,
      Some(keys),
      Some(if enabled { !binding.enabled } else { false }),
      &self.keybindings,
    ) {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    self.keybinding_reload_deadline = Some(Instant::now() + Duration::from_millis(250));
    self.success(tr(self.lang, "control_center.keybindings_applied").to_string());
  }

  /// Executes the `restore_keybinding` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn restore_keybinding(&mut self) {
    let binding = self.edited_binding().or_else(|| self.selected_keybinding());
    let Some(binding) = binding else {
      return;
    };
    if let Err(error) = keybindings::restore(&binding.id, &self.keybindings) {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    let _ = std::process::Command::new("argvus-sessionctl")
      .arg("reload")
      .status();
    self.success(tr(self.lang, "control_center.keybindings_restored").to_string());
  }

  /// Executes the `begin_keybinding_capture` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn begin_keybinding_capture(&mut self) {
    if let Some(keys) = self
      .edited_binding()
      .or_else(|| self.selected_keybinding())
      .map(|binding| binding.keys.clone())
    {
      let parts: Vec<&str> = keys.split(" + ").collect();
      self.keybinding_super_required = parts.contains(&"SUPER");
      self.keybinding_editor_modifiers = [
        parts.contains(&"CTRL"),
        parts.contains(&"ALT"),
        parts.contains(&"SHIFT"),
        parts.contains(&"SUPER"),
      ];
      self.keybinding_editor_key = parts.last().copied().unwrap_or_default().to_string();
    }
    self.keybinding_editor_field = 0;
    self.keybinding_capturing = true;
    self.keybinding_detected = false;
    self.success(tr(self.lang, "control_center.keybindings_press_key").to_string());
  }
  /// Checks the condition represented by `is_keybinding_capturing` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn is_keybinding_capturing(&self) -> bool {
    self.keybinding_capturing
  }
  /// Checks the condition represented by `has_keybinding_conflict` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn has_keybinding_conflict(&self) -> bool {
    self.keybinding_conflict.is_some()
  }
  /// Executes the `keybinding_conflict_state` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn keybinding_conflict_state(&self) -> Option<(&str, &str, Vec<String>)> {
    self
      .keybinding_conflict
      .as_ref()
      .map(|(id, keys, conflicts)| (id.as_str(), keys.as_str(), conflicts.clone()))
  }
  /// Executes the `keybinding_label_for` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn keybinding_label_for(&self, id: &str) -> String {
    self
      .keybindings
      .iter()
      .find(|binding| binding.id == id)
      .map(|binding| keybinding_label(self.lang, binding))
      .unwrap_or_else(|| id.to_string())
  }
  /// Executes the `cancel_keybinding_conflict` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn cancel_keybinding_conflict(&mut self) {
    self.keybinding_conflict = None;
  }
  /// Executes the `replace_keybinding_conflict` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn replace_keybinding_conflict(&mut self) {
    let Some((id, keys, conflicts)) = self.keybinding_conflict.take() else {
      return;
    };
    for old_id in conflicts {
      let _ = keybindings::save_override(&old_id, None, Some(false), &self.keybindings);
    }
    if let Err(error) = keybindings::save_override(&id, Some(keys), Some(true), &self.keybindings) {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    let _ = std::process::Command::new("argvus-sessionctl")
      .arg("reload")
      .status();
    self.success(tr(self.lang, "control_center.keybindings_replaced").to_string());
    if self.page() == Page::KeybindingCapture {
      self.navigation.back();
    }
    self.navigation.back();
  }
  /// Executes the `capture_keybinding` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn capture_keybinding(&mut self, key: crossterm::event::KeyEvent) {
    if key.code == crossterm::event::KeyCode::Esc {
      self.keybinding_capturing = false;
      self.keybinding_detected = false;
      return;
    }
    if self.keybinding_detected {
      if key.code == crossterm::event::KeyCode::Enter {
        self.apply_keybinding_editor();
      }
      return;
    }
    if key.code == crossterm::event::KeyCode::Enter && self.keybinding_editor_key.is_empty() {
      self.keybinding_editor_key = "Return".into();
    } else if key.code == crossterm::event::KeyCode::Tab && self.keybinding_editor_key.is_empty() {
      self.keybinding_editor_key = "Tab".into();
    } else if key.code == crossterm::event::KeyCode::Backspace
      && self.keybinding_editor_key.is_empty()
    {
      self.keybinding_editor_key = "BackSpace".into();
    } else {
      self.capture_keybinding_key(key);
    }
    if !self.keybinding_editor_key.is_empty() {
      self.keybinding_detected = true;
      self.keybinding_capturing = false;
    }
  }

  /// Executes the `capture_keybinding_key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn capture_keybinding_key(&mut self, key: crossterm::event::KeyEvent) {
    if key.code == crossterm::event::KeyCode::Enter {
      self.apply_keybinding_editor();
      return;
    }
    self.keybinding_editor_modifiers = [
      key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL),
      key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
      key
        .modifiers
        .contains(crossterm::event::KeyModifiers::SHIFT),
      self.keybinding_super_required
        || key
          .modifiers
          .contains(crossterm::event::KeyModifiers::SUPER),
    ];
    let raw = match key.code {
      crossterm::event::KeyCode::Char(c) => match c {
        '?' => "/".into(),
        '_' => "-".into(),
        '+' => "=".into(),
        ':' => ";".into(),
        '"' => "'".into(),
        '{' => "[".into(),
        '}' => "]".into(),
        '|' => "\\".into(),
        '~' => "`".into(),
        '<' => ",".into(),
        '>' => ".".into(),
        other => other.to_string(),
      },
      crossterm::event::KeyCode::Enter => "Return".into(),
      crossterm::event::KeyCode::Tab => "Tab".into(),
      crossterm::event::KeyCode::Backspace => "BackSpace".into(),
      crossterm::event::KeyCode::Left => "left".into(),
      crossterm::event::KeyCode::Right => "right".into(),
      crossterm::event::KeyCode::Up => "up".into(),
      crossterm::event::KeyCode::Down => "down".into(),
      crossterm::event::KeyCode::Esc => "Escape".into(),
      crossterm::event::KeyCode::Delete => "Delete".into(),
      crossterm::event::KeyCode::Insert => "Insert".into(),
      crossterm::event::KeyCode::Home => "Home".into(),
      crossterm::event::KeyCode::End => "End".into(),
      crossterm::event::KeyCode::PageUp => "Page_Up".into(),
      crossterm::event::KeyCode::PageDown => "Page_Down".into(),
      crossterm::event::KeyCode::F(n) => format!("F{n}"),
      _ => return,
    };
    self.keybinding_editor_key = raw;
  }

  /// Applies the `apply_keybinding_editor` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn apply_keybinding_editor(&mut self) {
    let mut parts = Vec::new();
    for (enabled, modifier) in self
      .keybinding_editor_modifiers
      .into_iter()
      .zip(["CTRL", "ALT", "SHIFT", "SUPER"])
    {
      if enabled {
        parts.push(modifier);
      }
    }
    if self.keybinding_editor_key.is_empty() {
      return;
    }
    parts.push(self.keybinding_editor_key.as_str());
    let Ok(keys) = keybindings::normalize_keys(&parts.join(" + ")) else {
      return;
    };
    let binding = self.edited_binding().or_else(|| {
      self
        .visible_keybindings()
        .get(self.navigation.current().selected)
        .copied()
    });
    let Some(binding) = binding else {
      return;
    };
    let conflicts = keybindings::conflicts_in_context(
      &self.keybindings,
      &binding.id,
      &keys,
      &binding.context,
      &binding.input,
    );
    if !conflicts.is_empty() {
      self.keybinding_conflict = Some((binding.id.clone(), keys, conflicts));
      self.keybinding_capturing = false;
      return;
    }
    let id = binding.id.clone();
    if let Err(error) = keybindings::save_override(&id, Some(keys), Some(true), &self.keybindings) {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    self.keybinding_capturing = false;
    let _ = std::process::Command::new("argvus-sessionctl")
      .arg("reload")
      .status();
    self.success(tr(self.lang, "control_center.keybindings_applied").to_string());
    if self.page() == Page::KeybindingCapture {
      self.navigation.back();
    }
    self.navigation.back();
  }

  /// Executes the `keybinding_editor_state` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn keybinding_editor_state(&self) -> ([bool; 4], &str, usize) {
    (
      self.keybinding_editor_modifiers,
      &self.keybinding_editor_key,
      self.keybinding_editor_field,
    )
  }

  /// Executes the `ensure_visible` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `setting_options` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `default_detail` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default_detail(&self, value: String, default: bool) -> String {
    if default {
      format!(
        "{value} [{}]",
        tr(self.lang, "control_center.argvus_default_5cdad7")
      )
    } else {
      value
    }
  }

  /// Executes the `success` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn success(&mut self, text: String) {
    self.status = Some(Status {
      text,
      kind: StatusKind::Success,
      created: Instant::now(),
    });
  }

  /// Executes the `fail` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Executes the `keybinding_is_cheatsheet_entry` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keybinding_is_cheatsheet_entry(binding: &keybindings::Binding) -> bool {
  !matches!(
    binding.id.as_str(),
    "resize.right"
      | "resize.left"
      | "resize.up"
      | "resize.down"
      | "resize.move_right"
      | "resize.move_left"
      | "resize.move_up"
      | "resize.move_down"
      | "resize.cancel_escape"
      | "resize.cancel_return"
  )
}

/// Executes the `keybinding_section` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keybinding_section(lang: Lang, binding: &keybindings::Binding) -> String {
  let software = matches!(
    binding.id.as_str(),
    id if id.starts_with("app.")
      || matches!(
        id,
        "system.kitty_cheatsheet"
          | "system.hyprland_cheatsheet"
          | "system.clipboard"
          | "system.clipboard_clear"
          | "system.color_picker"
          | "system.emoji_picker"
          | "system.control_center"
      )
  );
  let media = matches!(
    binding.id.as_str(),
    "session.volume_up"
      | "session.volume_down"
      | "session.mute"
      | "session.brightness_up"
      | "session.brightness_down"
      | "session.play_pause"
      | "session.next_track"
      | "session.previous_track"
      | "session.stop_track"
  );
  let section_key = if media {
    "control_center.keybindings.section.media"
  } else if software {
    "control_center.keybindings.section.software"
  } else {
    "control_center.keybindings.section.desktop"
  };
  tr(lang, section_key).to_string()
}

/// Executes the `keybinding_section_order` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keybinding_section_order(binding: &keybindings::Binding) -> usize {
  if matches!(
    binding.id.as_str(),
    "session.volume_up"
      | "session.volume_down"
      | "session.mute"
      | "session.brightness_up"
      | "session.brightness_down"
      | "session.play_pause"
      | "session.next_track"
      | "session.previous_track"
      | "session.stop_track"
  ) {
    return 2;
  }
  if binding.id.starts_with("app.")
    || matches!(
      binding.id.as_str(),
      "system.kitty_cheatsheet"
        | "system.hyprland_cheatsheet"
        | "system.clipboard"
        | "system.clipboard_clear"
        | "system.color_picker"
        | "system.emoji_picker"
        | "system.control_center"
    )
  {
    1
  } else {
    0
  }
}

/// Executes the `keybinding_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keybinding_label(lang: Lang, binding: &keybindings::Binding) -> String {
  if let Some(description) = keybindings::cheatsheet_description(lang, binding) {
    return description;
  }
  let translated = tr(lang, &binding.description_key);
  if translated != binding.description_key {
    translated.to_string()
  } else {
    let label = binding
      .id
      .rsplit('.')
      .next()
      .unwrap_or(&binding.id)
      .split('_')
      .map(|part| {
        let mut chars = part.chars();
        match chars.next() {
          Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
          None => String::new(),
        }
      })
      .collect::<Vec<_>>()
      .join(" ");
    match label.as_str() {
      "Terminal" => "Open terminal".into(),
      "Browser" => "Open default browser".into(),
      "File Manager" => "Open file manager".into(),
      "Launcher" => "Open application launcher".into(),
      "Close" => "Close window".into(),
      "Maximize" => "Maximize window".into(),
      "Next" => "Next workspace".into(),
      "Previous" => "Previous workspace".into(),
      _ => label,
    }
  }
}

/// Executes the `keybinding_category` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keybinding_category(lang: Lang, category: &str) -> String {
  let key = format!("control_center.keybindings.category.{category}");
  let translated = tr(lang, &key);
  if translated == key {
    category.to_string()
  } else {
    translated.to_string()
  }
}

/// Executes the `keybinding_category_order` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keybinding_category_order(category: &str) -> usize {
  [
    "applications",
    "windows",
    "navigation",
    "workspaces",
    "system",
    "session",
    "media",
    "screenshots",
    "appearance",
    "widgets",
  ]
  .iter()
  .position(|value| *value == category)
  .unwrap_or(usize::MAX)
}

impl Row {
  /// Executes the `plain` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn plain(label: &str) -> Self {
    Self {
      label: label.to_string(),
      detail: None,
      current: false,
    }
  }
}

/// Executes the `search_matches` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn search_matches(query: &str, values: &[&str]) -> bool {
  query.is_empty()
    || values
      .iter()
      .any(|value| value.to_lowercase().contains(&query.to_lowercase()))
}

/// Executes the `category_icon` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn category_icon(category: Category) -> &'static str {
  match category {
    Category::Terminal => argvus_tui::icons::MONITOR,
    Category::FileManager => argvus_tui::icons::FOLDER,
    Category::TextEditor => argvus_tui::icons::TEXT_EDITOR,
    Category::TerminalEditor => argvus_tui::icons::KEYBOARD,
    Category::Browser => argvus_tui::icons::NETWORK,
    Category::ImageViewer => argvus_tui::icons::IMAGE,
    Category::PdfViewer => argvus_tui::icons::PDF,
    Category::VideoPlayer => argvus_tui::icons::VIDEO,
    Category::AudioPlayer => argvus_tui::icons::MUSIC,
    Category::Archive => argvus_tui::icons::PACKAGES,
    Category::Launcher => argvus_tui::icons::BOOT,
  }
}

/// Executes the `category_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn category_label(lang: Lang, category: Category) -> &'static str {
  let key = match category {
    Category::Terminal => "control_center.terminal",
    Category::FileManager => "control_center.file_manager",
    Category::TextEditor => "control_center.text_editor",
    Category::TerminalEditor => "control_center.terminal_editor",
    Category::Browser => "control_center.browser",
    Category::ImageViewer => "control_center.image_viewer",
    Category::PdfViewer => "control_center.pdf_viewer",
    Category::VideoPlayer => "control_center.video_player",
    Category::AudioPlayer => "control_center.audio_player",
    Category::Archive => "control_center.archive",
    Category::Launcher => "control_center.launcher",
  };
  crate::i18n::tr(lang, key)
}

/// Executes the `font_target_icon` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn font_target_icon(target: FontTarget) -> &'static str {
  match target {
    FontTarget::Taskbar => argvus_tui::icons::MONITOR,
    FontTarget::Sysinfo => argvus_tui::icons::DIAGNOSTICS,
    FontTarget::ControlPanel => argvus_tui::icons::SETTINGS,
    FontTarget::System => argvus_tui::icons::SERVICES,
    FontTarget::Apps => argvus_tui::icons::PACKAGES,
    FontTarget::Terminal => argvus_tui::icons::KEYBOARD,
    FontTarget::Browser => argvus_tui::icons::NETWORK,
  }
}

/// Executes the `setting_icon` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn setting_icon(setting: SettingKind) -> &'static str {
  match setting {
    SettingKind::Antialiasing => argvus_tui::icons::SUCCESS,
    SettingKind::Hinting => argvus_tui::icons::SEARCH,
    SettingKind::Subpixel => argvus_tui::icons::PALETTE,
    SettingKind::Dpi => argvus_tui::icons::STORAGE,
  }
}

/// Executes the `font_target_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn font_target_label(lang: Lang, target: FontTarget) -> &'static str {
  match target {
    FontTarget::Taskbar => tr(lang, "control_center.taskbar_font"),
    FontTarget::Sysinfo => tr(lang, "control_center.widget_telemetry_font"),
    FontTarget::ControlPanel => tr(lang, "control_center.control_panel_font"),
    FontTarget::System => tr(lang, "control_center.system_font"),
    FontTarget::Apps => tr(lang, "control_center.applications_font"),
    FontTarget::Terminal => tr(lang, "control_center.terminal_font"),
    FontTarget::Browser => tr(lang, "control_center.browser_font"),
  }
}

/// Executes the `setting_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn setting_label(lang: Lang, setting: SettingKind) -> &'static str {
  match setting {
    SettingKind::Antialiasing => tr(lang, "control_center.antialiasing"),
    SettingKind::Hinting => "Hinting",
    SettingKind::Subpixel => tr(lang, "control_center.subpixel_order"),
    SettingKind::Dpi => "DPI",
  }
}

/// Executes the `pending_action_text` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn pending_action_text(lang: Lang, action: &PendingAction) -> (String, String) {
  match action {
    PendingAction::Administration(body) => (
      tr(lang, "control_center.confirm_administrative_operation").into(),
      body.clone(),
    ),
    PendingAction::ResetApps => (
      tr(lang, "control_center.reset_default_applications").to_string(),
      tr(
        lang,
        "control_center.this_will_restore_the_application_choices_defined_by_argvus",
      )
      .to_string(),
    ),
    PendingAction::ResetApp(category) => (
      format!("{}?", tr(lang, "control_center.reset_default_application")),
      category_label(lang, *category).to_string(),
    ),
    PendingAction::ResetFonts => (
      tr(lang, "control_center.reset_font_settings").to_string(),
      tr(
        lang,
        "control_center.this_will_restore_the_font_settings_defined_by_argvus",
      )
      .to_string(),
    ),
    PendingAction::ResetFont(target) => (
      format!("{}?", tr(lang, "control_center.reset_font")),
      font_target_label(lang, *target).to_string(),
    ),
    PendingAction::ResetFontSetting(setting) => (
      format!("{}?", tr(lang, "control_center.reset_setting")),
      setting_label(lang, *setting).to_string(),
    ),
    PendingAction::ResetKeybindings => (
      tr(lang, "control_center.restore_all_shortcuts").to_string(),
      tr(lang, "control_center.restore_all_shortcuts_description").to_string(),
    ),
    PendingAction::ApplySystemLocales => (
      tr(lang, "control_center.apply_locale_changes").to_string(),
      tr(
        lang,
        "control_center.this_will_update_etc_locale_gen_and_run_locale_gen",
      )
      .to_string(),
    ),
    PendingAction::SetNtp(enabled) => (
      tr(lang, "control_center.change_automatic_date_time").to_string(),
      enabled_label(lang, *enabled).to_string(),
    ),
  }
}

/// Executes the `setting_value_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn setting_value_label(lang: Lang, value: &str) -> String {
  match value {
    "enabled" => tr(lang, "control_center.enabled_16b283").to_string(),
    "disabled" => tr(lang, "control_center.disabled_8ccfd8").to_string(),
    "none" => tr(lang, "control_center.none").to_string(),
    "slight" => tr(lang, "control_center.slight").to_string(),
    "medium" => tr(lang, "control_center.medium").to_string(),
    "full" => tr(lang, "control_center.full").to_string(),
    "automatic" => tr(lang, "control_center.automatic_b0d36e").to_string(),
    _ => value.to_string(),
  }
}

/// Executes the `enabled_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn enabled_label(lang: Lang, enabled: bool) -> &'static str {
  if enabled {
    tr(lang, "control_center.enabled")
  } else {
    tr(lang, "control_center.disabled")
  }
}

/// Executes the `non_empty` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn non_empty(value: &str) -> String {
  if value.is_empty() {
    "-".to_string()
  } else {
    value.to_string()
  }
}

/// Defines the constant `TASK_POPUP_WIDTH`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub(crate) const TASK_POPUP_WIDTH: u16 = 100;
/// Defines the constant `TASK_POPUP_HEIGHT`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub(crate) const TASK_POPUP_HEIGHT: u16 = 20;
/// Defines the constant `TASK_CONTENT_WIDTH`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub(crate) const TASK_CONTENT_WIDTH: usize = TASK_POPUP_WIDTH as usize - 2;
/// Defines the constant `TASK_CONTENT_HEIGHT`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub(crate) const TASK_CONTENT_HEIGHT: usize = TASK_POPUP_HEIGHT as usize - 2;

/// Executes the `task_wrapped_lines` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Executes the `task_bottom_offset` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub(crate) fn task_bottom_offset(output: &str) -> u16 {
  task_wrapped_lines(output).saturating_sub(TASK_CONTENT_HEIGHT) as u16
}

/// Executes the `language_from_selected` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn language_from_selected(selected: usize) -> Lang {
  if selected.saturating_sub(LANGUAGE_INFO_ROWS) == 1 {
    Lang::for_locale("pt-BR")
  } else {
    Lang::for_locale("en-US")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  /// Executes the `search_is_case_insensitive` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn search_is_case_insensitive() {
    assert!(search_matches("nOtO", &["Noto Sans", "Regular"]));
    assert!(!search_matches("Roboto", &["Noto Sans"]));
  }

  #[test]
  /// Executes the `hardware_rows_follow_ratbag_capabilities` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn hardware_rows_follow_ratbag_capabilities() {
    let mut app = App::with_context(
      Page::MouseTouchpad,
      Lang::for_locale("en-US"),
      Theme::load(),
    );
    app.input_loading = false;
    app.input.devices.mouse = true;
    app.input.ratbag = vec![crate::system::ratbag::Device {
      path: "/device/test".into(),
      name: "Test mouse".into(),
      profiles: vec![crate::system::ratbag::Profile {
        path: "/profile/test".into(),
        name: String::new(),
        index: 3,
        active: true,
        disabled: false,
        report_rate: Some(500),
        report_rates: vec![125, 500],
        resolutions: vec![crate::system::ratbag::Resolution {
          path: "/resolution/test".into(),
          index: 2,
          dpi_x: 800,
          dpi_y: 800,
          active: true,
          default: true,
          supported: vec![400, 800],
        }],
      }],
    }];
    let rows = app.rows();
    assert!(
      rows
        .iter()
        .any(|row| { row.label == tr(app.lang, "control_center.input_hardware_mouse") })
    );
    let dpi_label = tr(app.lang, "control_center.input_hardware_dpi");
    let polling_label = tr(app.lang, "control_center.input_hardware_polling_rate");
    assert!(rows.iter().any(|row| row.label == dpi_label));
    assert!(rows.iter().any(|row| row.label == polling_label));
    assert!(app.row_selectable(rows.iter().position(|row| row.label == dpi_label).unwrap()));
  }

  #[test]
  /// Executes the `labels_cover_every_backend_category` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn labels_cover_every_backend_category() {
    for category in Category::ORDER {
      assert!(!category_label(Lang::for_locale("en-US"), category).is_empty());
      assert!(!category_label(Lang::for_locale("pt-BR"), category).is_empty());
    }
  }

  #[test]
  /// Executes the `language_page_shows_status_lines_and_selectable_choices` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn language_page_shows_status_lines_and_selectable_choices() {
    let app = App::with_context(Page::Language, Lang::for_locale("en-US"), Theme::load());
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
  /// Executes the `language_selection_maps_to_languages` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn language_selection_maps_to_languages() {
    assert_eq!(
      language_from_selected(LANGUAGE_INFO_ROWS),
      Lang::for_locale("en-US")
    );
    assert_eq!(
      language_from_selected(LANGUAGE_INFO_ROWS + 1),
      Lang::for_locale("pt-BR")
    );
    assert_eq!(language_from_selected(0), Lang::for_locale("en-US"));
  }

  #[test]
  /// Executes the `fonts_dashboard_uses_icons_and_status` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn fonts_dashboard_uses_icons_and_status() {
    let app = App::with_context(Page::Fonts, Lang::for_locale("en-US"), Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), 11, "7 font targets + 4 settings");
    assert!(rows[0].label.contains("Taskbar"), "{}", rows[0].label);
    assert!(
      rows[0].label.contains(argvus_tui::icons::MONITOR),
      "{}",
      rows[0].label
    );
    assert!(rows[0].detail.is_some(), "font target should have detail");
    assert!(rows[7].label.contains("Antialiasing"), "{}", rows[7].label);
    assert!(
      rows[7].label.contains(argvus_tui::icons::SUCCESS),
      "{}",
      rows[7].label
    );
    for (i, row) in rows.iter().enumerate() {
      assert!(row.detail.is_some(), "row {i} should have detail");
    }
  }

  #[test]
  /// Executes the `fonts_dashboard_icons_are_distinct` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
  /// Executes the `default_apps_dashboard_uses_icons_and_status` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default_apps_dashboard_uses_icons_and_status() {
    let app = App::with_context(Page::DefaultApps, Lang::for_locale("en-US"), Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), Category::ORDER.len());
    assert!(rows[0].label.contains("Terminal"), "{}", rows[0].label);
    assert!(
      rows[0].label.contains(argvus_tui::icons::MONITOR),
      "{}",
      rows[0].label
    );
    assert!(rows[0].detail.is_some(), "category should have detail");
    assert!(rows[2].label.contains("Editor"), "{}", rows[2].label);
    assert!(rows[4].label.contains("Browser"), "{}", rows[4].label);
    for (i, row) in rows.iter().enumerate() {
      assert!(row.detail.is_some(), "row {i} should have detail");
    }
  }

  #[test]
  /// Executes the `default_apps_dashboard_icons_are_distinct` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default_apps_dashboard_icons_are_distinct() {
    let mut seen = std::collections::HashSet::new();
    for category in Category::ORDER {
      let icon = category_icon(category);
      assert!(seen.insert(icon), "duplicate icon {icon} for {category:?}");
    }
  }

  #[test]
  /// Executes the `system_dashboard_uses_icons_and_live_state` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn system_dashboard_uses_icons_and_live_state() {
    let app = App::with_context(Page::System, Lang::for_locale("en-US"), Theme::load());
    let rows = app.rows();
    assert_eq!(rows.len(), 4);
    assert!(rows[0].label.contains("Hostname"), "{}", rows[0].label);
    assert!(
      rows[0].label.contains(argvus_tui::icons::MONITOR),
      "{}",
      rows[0].label
    );
    assert!(rows[0].detail.is_some(), "hostname should have detail");
    assert_eq!(rows[1].label, format!("{} Users", argvus_tui::icons::USER));
    assert_eq!(rows[1].detail.as_deref(), Some("N/A"));
    assert_eq!(
      rows[2].label,
      format!("{} Groups:", argvus_tui::icons::USERS)
    );
    assert_eq!(rows[2].detail.as_deref(), Some("N/A"));
    assert!(rows[3].label.contains("Do Not Disturb"));
    for (index, row) in rows.iter().enumerate() {
      assert!(row.detail.is_some(), "row {index} should have detail");
    }
  }

  #[test]
  /// Executes the `hostname_page_row_keeps_current_value_while_editing` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn hostname_page_row_keeps_current_value_while_editing() {
    let mut app = App::with_context(Page::Hostname, Lang::for_locale("en-US"), Theme::load());
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
  /// Executes the `system_dashboard_counts_match_loaded_accounts` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn system_dashboard_counts_match_loaded_accounts() {
    let mut app = App::with_context(Page::System, Lang::for_locale("en-US"), Theme::load());
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
  /// Executes the `task_window_scrolls_and_closes` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn task_window_scrolls_and_closes() {
    let mut app = App::with_context(Page::Main, Lang::for_locale("en-US"), Theme::load());
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
  /// Executes the `task_bottom_offset_is_zero_for_short_output` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn task_bottom_offset_is_zero_for_short_output() {
    let mut app = App::with_context(Page::Main, Lang::for_locale("en-US"), Theme::load());
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
  /// Executes the `empty_poll_returns_false` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn empty_poll_returns_false() {
    let mut app = App::with_context(Page::Main, Lang::for_locale("en-US"), Theme::load());
    assert!(!app.poll());
  }
}
