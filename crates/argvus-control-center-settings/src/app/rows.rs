//! Menu rows of the Settings pages that are not account or firewall pages.
//!
//! Each page builds typed rows (`argvus_tui::menu::Row<Item>`): the row kind
//! decides focus and the activation event, and the icon belongs to the item.
use argvus_control_center_apps::catalog::Category;
use argvus_tui::icons;
use argvus_tui::menu::{Row, draft_actions};

use super::{
  App, keybinding_category, keybinding_label, keybinding_section, non_empty, search_matches,
  setting_value_label,
};
use crate::config::fonts::{FontTarget, SettingKind};
use crate::i18n::{Lang, na, tr};
use crate::item::{Item, RatbagRow};
use crate::navigation::Page;
use crate::system::{keybindings, locale};

/// Languages offered by the Language page, in display order.
pub(super) const LANGUAGES: [(&str, &str, &str); 2] = [
  (
    "en-US",
    "control_center.language_english",
    "control_center.language_english_us",
  ),
  (
    "pt-BR",
    "control_center.language_portuguese",
    "control_center.language_portuguese_brazil",
  ),
];

impl App {
  fn label(&self, key: &str) -> &'static str {
    tr(self.lang, key)
  }

  /// The trailing Danger zone of a page: its title and the given rows.
  fn danger_zone(&self, rows: &mut Vec<Row<Item>>, danger: Vec<Row<Item>>) {
    rows.push(Row::section(self.label("control_center.danger_zone")));
    rows.extend(danger);
  }

  /// "Restore defaults" (D6): same confirmation as the `r` shortcut.
  fn reset_defaults_zone(&self, rows: &mut Vec<Row<Item>>) {
    let reset = Row::destructive(
      Item::ResetDefaults,
      self.label("control_center.reset_defaults"),
    )
    .icon(icons::RESTORE);
    self.danger_zone(rows, vec![reset]);
  }

  /// Shown in place of an empty data list.
  fn empty_list_row(&self, page: Page) -> Row<Item> {
    let message = if matches!(page, Page::AppSelector(_)) && self.search.is_empty() {
      "control_center.no_installed_applications_detected"
    } else {
      "control_center.no_matching_items"
    };
    Row::info(self.label(message), "")
  }

  /// Rows of the current page, in display order.
  pub fn rows(&self) -> Vec<Row<Item>> {
    let page = self.page();
    if crate::administration::is_page(page) {
      let rows = self.admin.rows(page, self.lang, &self.search);
      // Group create and edit are for administrators only; everyone else
      // sees the group pages with every row disabled.
      let group_writes = matches!(page, Page::CreateGroup | Page::Group | Page::GroupMembers);
      if group_writes && !self.admin.is_admin() {
        return rows.into_iter().map(|row| row.enabled(false)).collect();
      }
      return rows;
    }
    match page {
      Page::Main => vec![
        Row::submenu(Item::DefaultApps, self.label("control_center.default_apps"))
          .icon(icons::APPS),
        Row::submenu(Item::Fonts, self.label("control_center.fonts")).icon(icons::FONTS),
        Row::submenu(
          Item::LocaleRegion,
          self.label("control_center.locale_region"),
        )
        .icon(icons::EARTH),
        Row::submenu(Item::System, self.label("control_center.system")).icon(icons::SETTINGS),
      ],
      Page::DefaultApps => {
        let mut rows: Vec<Row<Item>> = Category::ORDER
          .into_iter()
          .map(|category| {
            Row::submenu(
              Item::Category(category),
              super::category_label(self.lang, category),
            )
            .icon(super::category_icon(category))
            .detail(
              self.default_detail(self.apps.current(category), self.apps.is_default(category)),
            )
          })
          .collect();
        self.reset_defaults_zone(&mut rows);
        rows
      }
      Page::AppSelector(category) => {
        let current = self.apps.current(category);
        let mut rows: Vec<Row<Item>> = self
          .apps
          .installed(category)
          .iter()
          .enumerate()
          .filter(|(_, app)| search_matches(&self.search, &[&app.display, &app.binary]))
          .map(|(index, app)| {
            Row::choice(Item::App(index), app.display.clone(), app.binary == current)
              .detail(app.binary.clone())
          })
          .collect();
        if rows.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        self.reset_defaults_zone(&mut rows);
        rows
      }
      Page::Fonts => {
        let mut rows = vec![Row::section(self.label("control_center.fonts"))];
        rows.extend(FontTarget::ALL.into_iter().map(|target| {
          let font = self.fonts.get(target);
          Row::submenu(
            Item::FontTarget(target),
            super::font_target_label(self.lang, target),
          )
          .icon(super::font_target_icon(target))
          .detail(format!("{} · {}", font.display_name(), font.size))
        }));
        rows.push(Row::section(self.label("control_center.section_rendering")));
        rows.extend(SettingKind::ALL.into_iter().map(|setting| {
          Row::submenu(
            Item::FontSetting(setting),
            super::setting_label(self.lang, setting),
          )
          .icon(super::setting_icon(setting))
          .detail(setting_value_label(
            self.lang,
            &self.fonts.setting_value(setting),
          ))
        }));
        self.reset_defaults_zone(&mut rows);
        rows
      }
      Page::FontSelector(target) => {
        let current = self.fonts.get(target);
        let mut rows = vec![
          Row::value(
            Item::FontSize,
            self.label("control_center.font_size"),
            self.pending_size.to_string(),
            Some(1),
          )
          .icon(icons::FONT_SIZE),
          Row::separator(),
        ];
        let fonts: Vec<Row<Item>> = self
          .system_fonts
          .iter()
          .enumerate()
          .filter(|(_, font)| search_matches(&self.search, &[&font.family, &font.style]))
          .map(|(index, font)| {
            Row::choice(
              Item::Font(index),
              font.display_name(),
              font.family == current.family && font.style == current.style,
            )
          })
          .collect();
        if fonts.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows.extend(fonts);
        self.reset_defaults_zone(&mut rows);
        rows
      }
      Page::SettingSelector(setting) => {
        let current = self.fonts.setting_value(setting);
        let mut rows: Vec<Row<Item>> = super::setting_values(setting)
          .iter()
          .enumerate()
          .map(|(index, value)| {
            Row::choice(
              Item::SettingOption(index),
              setting_value_label(self.lang, value),
              *value == current,
            )
            .detail(*value)
          })
          .collect();
        self.reset_defaults_zone(&mut rows);
        rows
      }
      Page::LocaleRegion => vec![
        Row::submenu(Item::TimeZone, self.label("control_center.time_zone"))
          .icon(icons::CLOCK)
          .detail(non_empty(&self.datetime.time_zone)),
        Row::submenu(Item::DateTime, self.label("control_center.date_time"))
          .icon(icons::CALENDAR)
          .detail(non_empty(&self.datetime.local_time)),
        Row::submenu(
          Item::RegionalLocale,
          self.label("control_center.regional_locale"),
        )
        .icon(icons::EARTH)
        .detail(locale::current_lang()),
        Row::submenu(
          Item::SystemLocales,
          self.label("control_center.system_locales"),
        )
        .icon(icons::TRANSLATE)
        .detail(format!(
          "{} / {}",
          self.selected_locales.len(),
          self.locale_gen_entries.len()
        )),
        Row::submenu(Item::Keyboard, self.label("control_center.keyboard"))
          .icon(icons::KEYBOARD)
          .detail(format!(
            "{}  ·  {}",
            non_empty(&self.keyboard_info.x11_layout),
            non_empty(&self.keyboard_info.x11_variant)
          )),
      ],
      Page::TimeZone => {
        let mut rows: Vec<Row<Item>> = self
          .timezones
          .iter()
          .enumerate()
          .filter(|(_, zone)| search_matches(&self.search, &[zone]))
          .map(|(index, zone)| {
            Row::choice(
              Item::Zone(index),
              zone.clone(),
              *zone == self.datetime.time_zone,
            )
          })
          .collect();
        if rows.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows
      }
      Page::DateTime => {
        let ntp = self.datetime.ntp.unwrap_or(false);
        vec![
          Row::value(
            Item::LocalDateTime,
            self.label("control_center.local_date_time"),
            self.datetime.local_time.clone(),
            None,
          )
          .icon(icons::CALENDAR)
          .enabled(!ntp),
          Row::info(
            self.label("control_center.time_zone"),
            self.datetime.time_zone.clone(),
          ),
          Row::toggle(
            Item::Ntp,
            self.label("control_center.automatic_date_time_ntp"),
            ntp,
          )
          .icon(icons::SATELLITE),
          Row::info(
            "RTC",
            if self.datetime.rtc_local.unwrap_or(false) {
              self.label("control_center.local")
            } else {
              "UTC"
            },
          ),
        ]
      }
      Page::RegionalLocale => {
        let current = locale::current_lang();
        let mut rows = vec![
          Row::info(
            self.label("control_center.current_locale"),
            non_empty(&current),
          ),
          Row::info(
            self.label("control_center.encoding"),
            non_empty(&locale::encoding_from_locale(&current)),
          ),
          Row::separator(),
        ];
        let locales: Vec<Row<Item>> = self
          .generated_locales
          .iter()
          .enumerate()
          .filter(|(_, value)| search_matches(&self.search, &[value]))
          .map(|(index, value)| Row::choice(Item::Locale(index), value.clone(), *value == current))
          .collect();
        if locales.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows.extend(locales);
        rows
      }
      Page::SystemLocales => {
        let mut rows: Vec<Row<Item>> = self
          .locale_gen_entries
          .iter()
          .enumerate()
          .filter(|(_, entry)| search_matches(&self.search, &[&entry.locale, &entry.encoding]))
          .map(|(index, entry)| {
            let key = format!("{} {}", entry.locale, entry.encoding);
            let on = self.selected_locales.contains(&key);
            Row::toggle(Item::LocaleGen(index), key, on)
          })
          .collect();
        if rows.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows.extend(self.locales_apply_rows());
        rows
      }
      Page::Keyboard => vec![
        Row::submenu(Item::KeyboardLayout, self.label("control_center.layout"))
          .icon(icons::KEYBOARD)
          .detail(non_empty(&self.keyboard_info.x11_layout)),
        Row::submenu(Item::KeyboardVariant, self.label("control_center.variant"))
          .icon(icons::KEYBOARD_VARIANT)
          .detail(non_empty(&self.keyboard_info.x11_variant)),
        Row::info(
          self.label("control_center.model"),
          non_empty(&self.keyboard_info.x11_model),
        ),
        Row::info(
          self.label("control_center.options"),
          non_empty(&self.keyboard_info.x11_options),
        ),
        Row::submenu(
          Item::ConsoleKeymap,
          self.label("control_center.console_keymap"),
        )
        .icon(icons::TERMINAL)
        .detail(non_empty(&self.keyboard_info.console_keymap)),
        Row::info(
          "Hyprland XKB",
          format!(
            "{} {} {}",
            non_empty(&self.keyboard_info.hypr_layout),
            non_empty(&self.keyboard_info.hypr_variant),
            non_empty(&self.keyboard_info.hypr_options)
          ),
        ),
      ],
      Page::KeyboardLayout => {
        let default_layout = self.default_keyboard_layout();
        let mut rows: Vec<Row<Item>> = self
          .keyboard_layouts
          .iter()
          .enumerate()
          .filter(|(_, layout)| search_matches(&self.search, &[&layout.code, &layout.description]))
          .map(|(index, layout)| {
            let enabled = self
              .keyboard_info
              .hypr_layout
              .split(',')
              .any(|selected| selected.trim() == layout.code);
            // The configured default is the first entry of the Hyprland
            // layout list, not the layout hyprctl reports as active.
            let detail = if layout.code == default_layout {
              format!(
                "{} · {}",
                layout.description,
                self.label("control_center.default")
              )
            } else {
              layout.description.clone()
            };
            Row::toggle(Item::Layout(index), layout.code.clone(), enabled).detail(detail)
          })
          .collect();
        if rows.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows
      }
      Page::KeyboardVariant => {
        let mut rows: Vec<Row<Item>> = self
          .keyboard_variants
          .iter()
          .enumerate()
          .filter(|(_, variant)| {
            search_matches(&self.search, &[&variant.code, &variant.description])
          })
          .map(|(index, variant)| {
            let label = if variant.code.is_empty() {
              self.label("control_center.default").to_string()
            } else {
              variant.code.clone()
            };
            Row::choice(
              Item::Variant(index),
              label,
              variant.code == self.keyboard_info.x11_variant,
            )
            .detail(variant.description.clone())
          })
          .collect();
        if rows.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows
      }
      Page::ConsoleKeymap => {
        let mut rows: Vec<Row<Item>> = self
          .console_keymaps
          .iter()
          .enumerate()
          .filter(|(_, keymap)| search_matches(&self.search, &[keymap]))
          .map(|(index, keymap)| {
            Row::choice(
              Item::Keymap(index),
              keymap.clone(),
              *keymap == self.keyboard_info.console_keymap,
            )
          })
          .collect();
        if rows.is_empty() {
          rows.push(self.empty_list_row(page));
        }
        rows
      }
      Page::Language => self.language_rows(),
      Page::System => self.system_rows(),
      Page::Hostname => vec![
        Row::value(
          Item::Hostname,
          self.label("control_center.current_hostname"),
          self.hostname.clone(),
          None,
        )
        .icon(icons::MONITOR),
      ],
      Page::MouseTouchpad => self.input_rows(),
      Page::Keybindings => self.keybinding_rows(),
      Page::WindowRules => self.window_rules_rows(),
      Page::KeybindingEdit => self.keybinding_edit_rows(),
      Page::KeybindingCapture => self.keybinding_capture_rows(),
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
      | Page::UserAvatar
      | Page::UserUsername
      | Page::UserFullName
      | Page::UserDelete
      | Page::Groups
      | Page::GroupList
      | Page::SystemGroups
      | Page::Group
      | Page::GroupMembers
      | Page::CreateGroup => unreachable!("account and firewall rows come from Administration"),
    }
  }

  /// `Apply` of the System Locales selection.
  fn locales_apply_rows(&self) -> Vec<Row<Item>> {
    let changed = self.locales_changed();
    let mut rows = draft_actions(
      Item::ApplyLocales,
      self.label("control_center.apply"),
      changed,
    );
    if changed && let Some(apply) = rows.pop() {
      rows.push(apply.detail(self.label("control_center.draft_changed")));
    }
    rows
  }

  /// Whether the marked locales differ from the ones enabled in
  /// `/etc/locale.gen`.
  pub(super) fn locales_changed(&self) -> bool {
    self.locale_gen_entries.iter().any(|entry| {
      entry.enabled
        != self
          .selected_locales
          .contains(&format!("{} {}", entry.locale, entry.encoding))
    }) || self.selected_locales.len()
      != self
        .locale_gen_entries
        .iter()
        .filter(|entry| entry.enabled)
        .count()
  }

  /// First layout of the Hyprland layout list: the configured default.
  pub(super) fn default_keyboard_layout(&self) -> &str {
    self
      .keyboard_info
      .hypr_layout
      .split(',')
      .map(str::trim)
      .find(|layout| !layout.is_empty())
      .unwrap_or_default()
  }

  fn system_rows(&self) -> Vec<Row<Item>> {
    let hostname = if self.hostname.is_empty() {
      na(self.lang).to_string()
    } else {
      self.hostname.clone()
    };
    let accounts_loaded = self.admin.is_loaded();
    let count = |count: usize| {
      if accounts_loaded {
        count.to_string()
      } else {
        na(self.lang).to_string()
      }
    };
    vec![
      Row::value(
        Item::Hostname,
        self.label("control_center.hostname"),
        hostname,
        None,
      )
      .icon(icons::MONITOR),
      Row::submenu(Item::Users, self.label("control_center.users"))
        .icon(icons::USER)
        .detail(count(self.admin.users(false).len())),
      Row::submenu(Item::Groups, self.label("control_center.groups_label"))
        .icon(icons::GROUP)
        .detail(count(self.admin.groups(false).len())),
      Row::toggle(
        Item::DoNotDisturb,
        self.label("control_center.do_not_disturb"),
        self.dnd_enabled,
      )
      .icon(icons::BELL_OFF),
    ]
  }

  fn language_rows(&self) -> Vec<Row<Item>> {
    let current_lang = locale::current_lang();
    let current_language = if self.lang.locale() == "pt-BR" {
      self.label("control_center.language_portuguese_brazil")
    } else {
      self.label("control_center.language_english_us")
    };
    let mut rows = vec![
      Row::info(
        self.label("control_center.current_language"),
        current_language,
      ),
      Row::info(
        self.label("control_center.regional_locale_90adc4"),
        non_empty(&current_lang),
      ),
      Row::info(
        self.label("control_center.encoding"),
        non_empty(&locale::encoding_from_locale(&current_lang)),
      ),
      Row::separator(),
    ];
    rows.extend(
      LANGUAGES
        .iter()
        .enumerate()
        .map(|(index, (locale, name, detail))| {
          Row::choice(
            Item::Language(index),
            self.label(name),
            self.lang == Lang::for_locale(locale),
          )
          .detail(self.label(detail))
        }),
    );
    rows
  }

  fn input_rows(&self) -> Vec<Row<Item>> {
    if self.input_loading {
      let loading = self.label("control_center.input_loading");
      let mut rows = vec![Row::section(self.label("control_center.input_mouse"))];
      rows.extend(
        [
          "control_center.input_pointer_speed",
          "control_center.input_acceleration",
          "control_center.input_natural_scrolling",
          "control_center.input_scroll_speed",
          "control_center.input_left_handed",
          "control_center.input_touchpad",
        ]
        .into_iter()
        .map(|key| Row::info(self.label(key), loading)),
      );
      return rows;
    }
    let mouse = &self.input.mouse;
    let touchpad = &self.input.touchpad;
    let has_mouse = self.input.devices.mouse;
    let mut rows = vec![
      Row::section(self.label("control_center.input_mouse")),
      Row::value(
        Item::Input(1),
        self.label("control_center.input_pointer_speed"),
        crate::system::input::speed_display(mouse.sensitivity),
        Some(1),
      )
      .enabled(has_mouse),
      Row::value(
        Item::Input(2),
        self.label("control_center.input_acceleration"),
        self.label(crate::system::input::accel_label(&mouse.accel_profile)),
        Some(1),
      )
      .enabled(has_mouse),
      Row::toggle(
        Item::Input(3),
        self.label("control_center.input_natural_scrolling"),
        mouse.natural_scroll,
      )
      .enabled(has_mouse),
      Row::value(
        Item::Input(4),
        self.label("control_center.input_scroll_speed"),
        crate::system::input::factor_display(mouse.scroll_factor),
        Some(1),
      )
      .enabled(has_mouse),
      Row::toggle(
        Item::Input(5),
        self.label("control_center.input_left_handed"),
        mouse.left_handed,
      )
      .enabled(has_mouse),
      Row::section(self.label("control_center.input_touchpad")).detail(
        if self.input.devices.touchpad {
          self.label("control_center.available")
        } else {
          self.label("control_center.not_available")
        },
      ),
    ];
    if self.input.devices.touchpad {
      rows.extend(
        [
          (
            7,
            "control_center.input_touchpad_natural_scrolling",
            touchpad.natural_scroll,
          ),
          (
            8,
            "control_center.input_tap_to_click",
            touchpad.tap_to_click,
          ),
          (
            9,
            "control_center.input_tap_and_drag",
            touchpad.tap_and_drag,
          ),
          (
            10,
            "control_center.input_two_finger_right_click",
            touchpad.two_finger_right_click,
          ),
          (
            11,
            "control_center.input_disable_while_typing",
            touchpad.disable_while_typing,
          ),
        ]
        .into_iter()
        .map(|(index, key, on)| Row::toggle(Item::Input(index), self.label(key), on)),
      );
    }
    rows.extend(self.ratbag_rows());
    rows
  }

  /// The hardware mouse shown on Mouse & Touchpad.
  fn ratbag_selected_device(&self) -> Option<&crate::system::ratbag::Device> {
    self
      .input
      .ratbag
      .get(self.ratbag_device)
      .or_else(|| self.input.ratbag.first())
  }

  fn ratbag_rows(&self) -> Vec<Row<Item>> {
    let Some(device) = self.ratbag_selected_device() else {
      return Vec::new();
    };
    let device_name = if device.name.is_empty() {
      self
        .label("control_center.input_hardware_device_unknown")
        .to_string()
    } else {
      device.name.clone()
    };
    let mut section = Row::section(self.label("control_center.input_hardware_mouse"));
    if self.input.ratbag.len() == 1 {
      section = section.detail(device_name.clone());
    }
    let mut rows = vec![section];
    if self.input.ratbag.len() > 1 {
      rows.push(Row::value(
        Item::Ratbag(RatbagRow::Device),
        self.label("control_center.input_hardware_device"),
        device_name,
        Some(1),
      ));
    }
    let Some(profile) = device.active_profile() else {
      return rows;
    };
    let profile_name = if profile.name.is_empty() {
      self.lang.tr_args(
        "control_center.input_hardware_profile_number",
        [("number", profile.index.to_string())],
      )
    } else {
      profile.name.clone()
    };
    rows.push(
      Row::value(
        Item::Ratbag(RatbagRow::Profile),
        self.label("control_center.input_hardware_profile"),
        profile_name,
        Some(1),
      )
      .enabled(device.profiles.len() > 1),
    );
    if profile.supports_dpi()
      && let Some(resolution) = profile.active_resolution()
    {
      let dpi = if resolution.dpi_x == resolution.dpi_y {
        resolution.dpi_x.to_string()
      } else {
        format!("{} × {}", resolution.dpi_x, resolution.dpi_y)
      };
      rows.push(Row::value(
        Item::Ratbag(RatbagRow::Dpi),
        self.label("control_center.input_hardware_dpi"),
        self
          .lang
          .tr_args("control_center.input_hardware_dpi_unit", [("value", dpi)]),
        Some(1),
      ));
    }
    if let Some(rate) = profile.report_rate
      && profile.supports_report_rate()
    {
      rows.push(Row::value(
        Item::Ratbag(RatbagRow::ReportRate),
        self.label("control_center.input_hardware_polling_rate"),
        self.lang.tr_args(
          "control_center.input_hardware_polling_unit",
          [("value", rate.to_string())],
        ),
        Some(1),
      ));
    }
    rows
  }

  fn keybinding_rows(&self) -> Vec<Row<Item>> {
    let mut rows = vec![Row::info(
      self.label("control_center.search_shortcuts"),
      self.search.clone(),
    )];
    let mut section = String::new();
    for index in self.visible_keybindings() {
      let binding = &self.keybindings[index];
      let binding_section = keybinding_section(self.lang, binding);
      if section != binding_section {
        section = binding_section.clone();
        rows.push(Row::section(binding_section));
        rows.push(Row::info(
          self.label("control_center.keybindings_column_shortcut"),
          self.label("control_center.keybindings_column_description"),
        ));
      }
      rows.push(
        Row::submenu(Item::Keybinding(index), binding_keys(self.lang, binding))
          .detail(keybinding_label(self.lang, binding)),
      );
    }
    let restore_all = Row::destructive(
      Item::RestoreAllShortcuts,
      self.label("control_center.restore_all_shortcuts"),
    )
    .icon(icons::RESTORE);
    self.danger_zone(&mut rows, vec![restore_all]);
    rows
  }

  fn window_rules_rows(&self) -> Vec<Row<Item>> {
    let mut rows = vec![Row::info(
      self.label("control_center.window_rules_hint"),
      String::new(),
    )];
    for (index, rule) in self.window_rules.iter().enumerate() {
      rows.push(Row::section(rule.name.clone()));
      rows.push(
        Row::action(
          Item::WindowRuleWorkspace(index),
          self.label("control_center.window_rules_workspace"),
        )
        .detail(rule.workspace.to_string()),
      );
      rows.push(
        Row::submenu(
          Item::WindowRuleClasses(index),
          self.label("control_center.window_rules_classes"),
        )
        .detail(rule.classes.join(", ")),
      );
    }
    rows.push(Row::action(
      Item::AddWindowRule,
      self.label("control_center.window_rules_add"),
    ));
    let remove = self
      .window_rules
      .iter()
      .enumerate()
      .map(|(index, rule)| {
        Row::destructive(
          Item::RemoveWindowRule(index),
          format!(
            "{} {}",
            self.label("control_center.window_rules_remove"),
            rule.name
          ),
        )
        .icon(icons::DELETE)
      })
      .collect();
    self.danger_zone(&mut rows, remove);
    rows
  }

  fn keybinding_edit_rows(&self) -> Vec<Row<Item>> {
    let Some(binding) = self.edited_binding() else {
      return Vec::new();
    };
    vec![
      Row::info(
        keybinding_label(self.lang, binding),
        keybinding_category(self.lang, &binding.category),
      ),
      Row::info(
        self.label("control_center.keybindings_current_shortcut"),
        binding_keys(self.lang, binding),
      ),
      Row::separator(),
      Row::action(
        Item::ChangeShortcut,
        self.label("control_center.change_shortcut"),
      )
      .icon(icons::EDIT),
      Row::action(Item::DisableShortcut, self.label("control_center.disable"))
        .icon(icons::KEYBOARD_OFF),
      Row::action(
        Item::RestoreShortcut,
        self.label("control_center.restore_default"),
      )
      .icon(icons::RESTORE),
    ]
  }

  fn keybinding_capture_rows(&self) -> Vec<Row<Item>> {
    let Some(binding) = self.edited_binding() else {
      return Vec::new();
    };
    let (status, shortcut) = if self.keybinding_detected {
      (
        "control_center.keybindings_detected_shortcut",
        self.captured_shortcut(),
      )
    } else {
      ("control_center.press_new_shortcut", String::new())
    };
    let apply = if self.keybinding_detected {
      Row::action(Item::ApplyCapture, self.label("control_center.apply")).icon(icons::APPLY)
    } else {
      Row::action(Item::ApplyCapture, self.label("control_center.try_again")).icon(icons::REFRESH)
    };
    vec![
      Row::info(keybinding_label(self.lang, binding), ""),
      Row::info(self.label(status), shortcut),
      Row::separator(),
      apply,
      Row::action(Item::CancelCapture, self.label("control_center.cancel")).icon(icons::CANCEL),
    ]
  }
}

/// Shortcut text of a binding, or "Disabled" when it is turned off.
fn binding_keys(lang: Lang, binding: &keybindings::Binding) -> String {
  if binding.enabled {
    keybindings::display_keys(&binding.keys)
  } else {
    tr(lang, "control_center.disabled").to_string()
  }
}
