//! Menu navigation and row activation of the Settings pages.
//!
//! Keys reach the page's [`MenuState`]; the resulting [`MenuEvent`] carries
//! the row's [`Item`], and the handlers below keep each option's original
//! effect and timing (immediate, confirmed, or draft).
use crossterm::event::KeyCode;

use argvus_control_center_apps::catalog::Category;
use argvus_tui::hints::{HintContext, confirm_hints, hints};
use argvus_tui::menu::{MenuEvent, MenuState, Row, RowKind};

use super::keyboard_draft;
use super::rows::LANGUAGES;
use super::{App, PendingAction, Status, StatusKind, keybinding_is_cheatsheet_entry};
use super::{
  keybinding_category_order, keybinding_label, keybinding_section, keybinding_section_order,
  search_matches,
};
use crate::administration::{EditTarget, Editor};
use crate::config::fonts::SettingKind;
use crate::i18n::{Lang, tr};
use crate::item::{Item, RatbagRow};
use crate::navigation::Page;
use crate::system::snippets;
use crate::system::window_rules;
use crate::system::{keybindings, keyboard, locale, time};

/// Values offered by a font rendering setting, in display order.
pub(crate) fn setting_values(setting: SettingKind) -> &'static [&'static str] {
  match setting {
    SettingKind::Antialiasing => &["enabled", "disabled"],
    SettingKind::Hinting => &["none", "slight", "medium", "full"],
    SettingKind::Subpixel => &["none", "rgb", "bgr", "vrgb", "vbgr"],
    SettingKind::Dpi => &[
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
  }
}

/// Pages whose list can be narrowed with `/`.
fn is_searchable(page: Page) -> bool {
  matches!(
    page,
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
  )
}

impl App {
  /// Keeps the cursor of the current page on a selectable row.
  pub fn normalize_selection(&mut self) {
    let rows = self.rows();
    self.navigation.current_mut().menu.normalize(&rows);
  }

  /// Identity of the selected row, if the page has one.
  pub fn selected_item(&self) -> Option<Item> {
    let rows = self.rows();
    let mut menu = self.navigation.current().menu;
    menu.normalize(&rows);
    menu.selected_id(&rows)
  }

  /// Kind of the selected row, used by the contextual footer.
  fn selected_kind(&self, rows: &[Row<Item>]) -> Option<RowKind> {
    let mut menu = self.navigation.current().menu;
    menu.normalize(rows);
    menu.selected_kind(rows)
  }

  /// Sends a key to the page's menu list and runs the resulting event.
  pub(crate) fn handle_menu_key(&mut self, key: KeyCode) {
    let mut rows = self.rows();
    if crate::administration::is_user_form(self.page()) {
      // The user page's list is the info block only; its buttons are driven
      // by Left/Right, so the cursor never walks into them.
      let actions_at = rows
        .iter()
        .position(|row| row.is_section())
        .unwrap_or(rows.len());
      rows.truncate(actions_at);
    }
    let page_size = self.viewport;
    let event = self
      .navigation
      .current_mut()
      .menu
      .handle(key, &rows, page_size);
    match event {
      MenuEvent::Activate(item) => self.activate(item),
      MenuEvent::Toggle(item) => self.toggle(item),
      MenuEvent::Adjust(item, delta) => self.adjust(item, delta),
      MenuEvent::Confirm(item) => self.confirm_item(item),
      MenuEvent::Back => self.back(),
      MenuEvent::Moved | MenuEvent::None => {}
    }
  }

  /// Runs the selected row, as Enter does.
  pub fn open_or_apply(&mut self) {
    self.handle_menu_key(KeyCode::Enter);
  }

  /// Enter/`→` on an Action, Submenu, Choice or Value row.
  pub(crate) fn activate(&mut self, item: Item) {
    if crate::administration::is_page(self.page()) {
      self.admin_activate(item);
      return;
    }
    match item {
      Item::DefaultApps => self.navigation.push(Page::DefaultApps),
      Item::Fonts => self.navigation.push(Page::Fonts),
      Item::LocaleRegion => self.navigation.push(Page::LocaleRegion),
      Item::System => self.navigation.push(Page::System),
      Item::Category(category) => {
        self.navigation.push(Page::AppSelector(category));
        self.select_current();
      }
      Item::App(index) => {
        if let Page::AppSelector(category) = self.page() {
          self.apply_app(category, index);
        }
      }
      Item::FontTarget(target) => {
        self.pending_size = self.fonts.get(target).size;
        self.navigation.push(Page::FontSelector(target));
        self.select_current();
      }
      Item::FontSetting(setting) => {
        self.navigation.push(Page::SettingSelector(setting));
        self.select_current();
      }
      Item::FontSize => {
        self.admin.editor = Some(Editor::new(
          tr(self.lang, "control_center.font_size").into(),
          self.pending_size.to_string(),
          EditTarget::FontSize,
          false,
        ));
      }
      Item::Font(index) => {
        if let Page::FontSelector(target) = self.page() {
          self.apply_font(target, index);
        }
      }
      Item::SettingOption(index) => {
        if let Page::SettingSelector(setting) = self.page() {
          self.apply_font_setting(setting, index);
        }
      }
      Item::TimeZone => self.open_system_page(Page::TimeZone),
      Item::DateTime => self.open_system_page(Page::DateTime),
      Item::RegionalLocale => self.open_system_page(Page::RegionalLocale),
      Item::SystemLocales => self.open_system_page(Page::SystemLocales),
      Item::Keyboard => self.open_system_page(Page::Keyboard),
      Item::WindowRuleName(index) => self.open_window_rule_name(index),
      Item::WindowRuleWorkspace(index) => self.adjust_window_rule(index, 1),
      Item::WindowRuleClasses(index) => self.open_window_rule_classes(index),
      Item::ApplyWindowRule(index) => self.apply_window_rule(index),
      Item::NewWindowRuleName => self.open_new_window_rule_name(),
      Item::AddWindowRule => self.add_window_rule(),
      Item::AddProjectPath => self.open_project_editor(false),
      Item::AddProjectRoot => self.open_project_editor(true),
      Item::NewSnippetName => self.open_new_snippet_name(),
      Item::NewSnippetContent => self.open_new_snippet_content(),
      Item::AddSnippet => self.add_snippet(),
      Item::OpenSnippetPicker => self.open_snippet_picker(),
      Item::SnippetContent(index) => self.open_snippet_content(index),
      Item::TypeSnippet(index) => self.type_snippet(index),
      Item::Zone(index) => self.apply_timezone(index),
      Item::LocalDateTime if !self.datetime.ntp.unwrap_or(false) => {
        self.admin.editor = Some(Editor::new(
          tr(self.lang, "control_center.local_date_time").into(),
          self.datetime.local_time.clone(),
          EditTarget::DateTime,
          false,
        ));
      }
      Item::Locale(index) => self.apply_regional_locale(index),
      Item::ApplyLocales => self.open_confirm(PendingAction::ApplySystemLocales),
      Item::KeyboardLayout => self.open_system_page(Page::KeyboardLayout),
      Item::KeyboardVariant => self.open_system_page(Page::KeyboardVariant),
      Item::ConsoleKeymap => self.open_system_page(Page::ConsoleKeymap),
      Item::Layout(index) => self.apply_keyboard_layout(index),
      Item::ApplyKeyboardLayout => self.apply_keyboard_layout_draft(),
      Item::Variant(index) => self.apply_keyboard_variant(index),
      Item::Keymap(index) => self.apply_console_keymap(index),
      Item::Language(index) => self.apply_language(index),
      Item::Hostname => {
        self.hostname_editing = true;
        self.hostname_input = self.hostname.clone();
      }
      Item::Users => {
        self.admin.load(false);
        self.navigation.push(Page::Users);
      }
      Item::Groups => {
        self.admin.load(false);
        self.navigation.push(Page::Groups);
      }
      Item::Input(_) | Item::Ratbag(_) => self.apply_input(item),
      Item::Keybinding(index) => self.open_keybinding_edit(index),
      Item::ChangeShortcut => {
        self.keybinding_capturing = true;
        self.keybinding_detected = false;
        self.navigation.push(Page::KeybindingCapture);
      }
      Item::DisableShortcut => {
        self.keybinding_capturing = false;
        self.disable_edited_keybinding();
        self.navigation.back();
      }
      Item::RestoreShortcut => {
        self.keybinding_capturing = false;
        self.restore_edited_keybinding();
        self.navigation.back();
      }
      Item::ApplyCapture => {
        if self.keybinding_detected {
          self.apply_keybinding_editor();
        } else {
          self.keybinding_capturing = true;
        }
      }
      Item::CancelCapture => {
        self.keybinding_capturing = false;
        self.navigation.back();
      }
      _ => {}
    }
  }

  /// Enter/Space on a Toggle row.
  fn toggle(&mut self, item: Item) {
    if crate::administration::is_page(self.page()) {
      self.admin_activate(item);
      return;
    }
    match item {
      Item::Ntp => self.open_confirm(PendingAction::SetNtp(!self.datetime.ntp.unwrap_or(false))),
      Item::DoNotDisturb => self.toggle_dnd(),
      Item::LocaleGen(index) => self.toggle_system_locale(index),
      Item::Layout(index) => self.toggle_keyboard_layout(index),
      Item::Input(_) => self.apply_input(item),
      _ => {}
    }
  }

  /// `←/→` on a Value row with a step.
  fn adjust(&mut self, item: Item, delta: i32) {
    if crate::administration::is_page(self.page()) {
      self.admin_adjust(item, delta);
      return;
    }
    match item {
      Item::FontSize => self.adjust_size(delta.signum() as i16),
      Item::Input(_) | Item::Ratbag(_) => self.input_cycle(item, delta.signum() as i8),
      Item::WindowRuleWorkspace(index) => self.adjust_window_rule(index, delta),
      _ => {}
    }
  }

  /// Enter on a Destructive row: every one of them asks first.
  fn confirm_item(&mut self, item: Item) {
    if crate::administration::is_page(self.page()) {
      self.admin_activate(item);
      return;
    }
    match item {
      Item::ResetDefaults => self.reset_current(),
      Item::RestoreAllShortcuts => self.open_confirm(PendingAction::ResetKeybindings),
      Item::RemoveWindowRule(index) => {
        if let Some(rule) = self.window_rules.get(index) {
          let name = rule.name.clone();
          self.open_confirm(PendingAction::RemoveWindowRule(name));
        }
      }
      Item::RemoveProject(index) => {
        if let Some(entry) = self.projects.entries.get(index) {
          let path = entry.path.clone();
          self.open_confirm(PendingAction::RemoveProject(path));
        }
      }
      Item::RemoveSnippet(index) => {
        if let Some(entry) = self.snippets.entries.get(index) {
          let name = entry.name.clone();
          self.open_confirm(PendingAction::RemoveSnippet(name));
        }
      }
      _ => {}
    }
  }

  /// The `r` shortcut. It restores defaults on the app and font pages and
  /// restores the selected shortcut on the shortcut pages, always through
  /// the confirmation (D6).
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
        if let Some(binding) = self.selected_keybinding() {
          let id = binding.id.clone();
          self.open_confirm(PendingAction::RestoreKeybinding { id, leave: false });
        }
      }
      Page::KeybindingEdit => {
        if let Some(id) = self.keybinding_edit_id.clone() {
          self.open_confirm(PendingAction::RestoreKeybinding { id, leave: true });
        }
      }
      _ => {}
    }
  }

  /// Whether the page's search is a box above the list, as on the home
  /// screen, instead of the bottom line.
  pub(crate) fn search_on_top(&self) -> bool {
    self.page() == Page::KeyboardLayout
  }

  /// The action bar of the current page; empty for pages without one.
  pub(crate) fn action_rows(&self) -> Vec<Row<Item>> {
    match self.page() {
      Page::KeyboardLayout => self.keyboard_action_rows(),
      _ => Vec::new(),
    }
  }

  /// Index of the focused action button, or `None` while the list has the focus.
  pub(crate) fn action_cursor(&self) -> Option<usize> {
    self.navigation.current().actions.cursor()
  }

  /// Indexes of the enabled buttons of the current page's action bar.
  fn enabled_action_indexes(&self) -> Vec<usize> {
    self
      .action_rows()
      .iter()
      .enumerate()
      .filter(|(_, row)| row.is_selectable())
      .map(|(index, _)| index)
      .collect()
  }

  /// `Tab`: moves the focus between the list and the action bar.
  pub(crate) fn toggle_action_focus(&mut self) {
    let enabled = self.enabled_action_indexes();
    self.navigation.current_mut().actions.toggle(&enabled);
  }

  /// `←/→` on the action bar: steps through its enabled buttons.
  pub(crate) fn move_action(&mut self, backwards: bool) {
    let enabled = self.enabled_action_indexes();
    self
      .navigation
      .current_mut()
      .actions
      .move_by(backwards, &enabled);
  }

  /// Gives the focus back to the list, as `↑/↓` do.
  pub(crate) fn focus_list(&mut self) {
    self.navigation.current_mut().actions.focus_list();
  }

  /// Enter/Space on the focused button: runs its row, as Enter on the row would.
  pub(crate) fn activate_action(&mut self) {
    let rows = self.action_rows();
    let Some(row) = self.action_cursor().and_then(|index| rows.get(index)) else {
      return;
    };
    if !row.is_selectable() {
      return;
    }
    if let Some(item) = row.id().copied() {
      self.activate(item);
    }
  }

  /// Leaves the search, or goes back one page.
  pub fn back(&mut self) {
    if self.searching || !self.search.is_empty() {
      self.searching = false;
      self.search.clear();
      self.navigation.current_mut().menu = MenuState::default();
      self.select_current();
      return;
    }
    self.hostname_editing = false;
    if self.leaving_drops_draft() {
      self.open_confirm(PendingAction::DiscardDraft);
      return;
    }
    self.navigation.back();
  }

  /// Whether going back from the current page would drop unsaved changes.
  fn leaving_drops_draft(&self) -> bool {
    match self.page() {
      Page::SystemLocales => self.locales_changed(),
      Page::KeyboardLayout => self.keyboard_layout_changed(),
      Page::WindowRules => self
        .window_rules
        .iter()
        .any(|rule| self.window_rule_changed(rule)),
      page => self.admin.leaving_drops_draft(page),
    }
  }

  /// Confirmed discard: the draft returns to the loaded state, then back.
  pub(super) fn discard_and_leave(&mut self) {
    match self.page() {
      Page::SystemLocales => {
        self.selected_locales = self
          .locale_gen_entries
          .iter()
          .filter(|entry| entry.enabled)
          .map(|entry| format!("{} {}", entry.locale, entry.encoding))
          .collect();
      }
      Page::KeyboardLayout => {
        self.keyboard_draft = keyboard_draft::LayoutDraft::loaded(&self.keyboard_info);
      }
      Page::WindowRules => {
        self.window_rule_drafts.clear();
        self.new_window_rule_name.clear();
      }
      page => self.admin.discard_draft(page),
    }
    self.navigation.back();
  }

  pub fn begin_search(&mut self) {
    if is_searchable(self.page()) {
      self.searching = true;
      self.search.clear();
      self.navigation.current_mut().menu = MenuState::default();
    }
  }

  pub fn push_search(&mut self, character: char) {
    self.search.push(character);
    self.navigation.current_mut().menu = MenuState::default();
  }

  pub fn pop_search(&mut self) {
    self.search.pop();
    self.navigation.current_mut().menu = MenuState::default();
  }

  /// Records the visible list height, the PgUp/PgDn distance.
  pub fn set_viewport(&mut self, viewport: usize) {
    self.viewport = viewport.max(1);
  }

  /// Puts the cursor on the current choice of the page (the active theme
  /// of a list, the default keyboard layout...), or on its first row.
  pub fn select_current(&mut self) {
    let rows = self.rows();
    let current = if self.page() == Page::KeyboardLayout {
      let default_layout = self.keyboard_draft.default.clone();
      self
        .keyboard_layouts
        .iter()
        .position(|layout| layout.code == default_layout)
        .map(Item::Layout)
    } else {
      rows
        .iter()
        .find(|row| matches!(row.kind(), RowKind::Choice { current: true }))
        .and_then(|row| row.id().copied())
    };
    let menu = &mut self.navigation.current_mut().menu;
    if !current.is_some_and(|item| menu.select(&rows, &item)) {
      menu.normalize(&rows);
    }
  }

  /// Footer derived from the selected row and the page's shortcuts.
  pub fn footer(&self) -> String {
    if self.confirm.is_some() {
      return confirm_hints(self.lang);
    }
    if self.has_keybinding_conflict() {
      return format!(
        "{}   r {}",
        confirm_hints(self.lang),
        tr(self.lang, "control_center.replace")
      );
    }
    if self.action_cursor().is_some() {
      // The bar's title already names `←/→`, `Enter` and `Tab`; the footer
      // shows what the focused button does and how to go back.
      return hints(
        self.lang,
        &HintContext {
          row: Some(RowKind::Action),
          can_go_back: true,
          ..HintContext::default()
        },
      );
    }
    if self.searching {
      return tr(
        self.lang,
        "control_center.type_to_search_enter_apply_esc_cancel",
      )
      .into();
    }
    let label = |key: &str| tr(self.lang, key);
    let page = self.page();
    let rows = self.rows();
    let mut row = self.selected_kind(&rows);
    let mut extra = Vec::new();
    match page {
      Page::DefaultApps
      | Page::AppSelector(_)
      | Page::Fonts
      | Page::FontSelector(_)
      | Page::SettingSelector(_) => extra.push(("r", label("control_center.reset_defaults"))),
      Page::Keybindings => {
        extra.push(("Space", label("control_center.hint.toggle")));
        extra.push(("r", label("control_center.restore_default")));
      }
      Page::KeybindingEdit => {
        extra.push(("e", label("control_center.change_shortcut")));
        extra.push(("r", label("control_center.restore_default")));
      }
      Page::WindowRules if matches!(self.selected_item(), Some(Item::WindowRuleWorkspace(_))) => {
        // `←/→` steps the workspace both ways, so the footer names it.
        extra.push(("←/→", label("control_center.window_rules_workspace")));
      }
      Page::KeyboardLayout if matches!(self.selected_item(), Some(Item::Layout(_))) => {
        // Enter sets the default layout and Space enables or disables it,
        // so the generic Toggle hint of the row does not apply here.
        row = Some(RowKind::Info);
        extra.push(("Enter", label("control_center.set_default")));
        extra.push(("Space", label("control_center.hint.toggle")));
      }
      _ => {}
    }
    hints(
      self.lang,
      &HintContext {
        row,
        can_go_back: page != Page::Main,
        search: is_searchable(page),
        refresh: matches!(
          page,
          Page::System | Page::UserList | Page::SystemUsers | Page::GroupList | Page::SystemGroups
        ),
        quit: page == Page::Main,
        extra: &extra,
      },
    )
  }

  fn apply_app(&mut self, category: Category, index: usize) {
    let Some((display, binary)) = self
      .apps
      .installed(category)
      .get(index)
      .map(|app| (app.display.clone(), app.binary.clone()))
    else {
      return;
    };
    match self.apps.set_default(category, &binary) {
      Ok(()) => self.success(format!(
        "{}: {display}",
        tr(self.lang, "control_center.default_application_changed")
      )),
      Err(error) => self.fail(error),
    }
  }

  fn apply_font(&mut self, target: crate::config::fonts::FontTarget, index: usize) {
    let Some(font) = self.system_fonts.get(index).cloned() else {
      return;
    };
    match self.fonts.apply_font(target, &font, self.pending_size) {
      Ok(()) => self.success(format!(
        "{}: {} {}",
        tr(self.lang, "control_center.font_applied"),
        font.display_name(),
        self.pending_size
      )),
      Err(error) => self.fail(error),
    }
  }

  fn apply_font_setting(&mut self, setting: SettingKind, index: usize) {
    let Some(value) = setting_values(setting).get(index) else {
      return;
    };
    match self.fonts.apply_setting(setting, value) {
      Ok(()) => self.success(tr(self.lang, "control_center.setting_applied").to_string()),
      Err(error) => self.fail(error),
    }
  }

  fn apply_timezone(&mut self, index: usize) {
    let Some(zone) = self.timezones.get(index).cloned() else {
      return;
    };
    match time::set_timezone(&zone, &self.timezones) {
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

  fn apply_regional_locale(&mut self, index: usize) {
    let Some(value) = self.generated_locales.get(index).cloned() else {
      return;
    };
    match locale::set_lang(&value, &self.generated_locales) {
      Ok(()) => self.success(format!(
        "{}: {value}",
        tr(self.lang, "control_center.regional_locale_changed")
      )),
      Err(error) => self.fail(error),
    }
  }

  /// Marks or unmarks one `/etc/locale.gen` entry; nothing is written until
  /// `Apply`.
  fn toggle_system_locale(&mut self, index: usize) {
    if let Some(entry) = self.locale_gen_entries.get(index) {
      let key = format!("{} {}", entry.locale, entry.encoding);
      if !self.selected_locales.remove(&key) {
        self.selected_locales.insert(key);
      }
    }
  }

  /// Enter on a layout: makes it the default in the draft. `Apply` writes it.
  fn apply_keyboard_layout(&mut self, index: usize) {
    let Some(code) = self
      .keyboard_layouts
      .get(index)
      .map(|layout| layout.code.clone())
    else {
      return;
    };
    self.keyboard_draft.set_default(&code);
  }

  /// Space on a layout: enables or disables it in the draft. `Apply` writes it.
  fn toggle_keyboard_layout(&mut self, index: usize) {
    let Some(code) = self
      .keyboard_layouts
      .get(index)
      .map(|layout| layout.code.clone())
    else {
      return;
    };
    if let Err(message) = self.keyboard_draft.toggle(&code) {
      self.fail(message);
    }
  }

  /// `Apply` of Keyboard > Layout: writes the draft in one backend call, then
  /// reloads the state so the draft matches what was written.
  fn apply_keyboard_layout_draft(&mut self) {
    let draft = self.keyboard_draft.clone();
    match keyboard::set_x11_layouts(&draft.default, &draft.selected, &self.keyboard_layouts) {
      Ok(()) => {
        self.refresh_keyboard();
        self.keyboard_draft = keyboard_draft::LayoutDraft::loaded(&self.keyboard_info);
        self.success(tr(self.lang, "control_center.keyboard_layouts_updated").to_string());
      }
      Err(error) => self.fail(error),
    }
  }

  fn apply_keyboard_variant(&mut self, index: usize) {
    let Some(variant) = self.keyboard_variants.get(index).cloned() else {
      return;
    };
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

  fn apply_console_keymap(&mut self, index: usize) {
    let Some(keymap) = self.console_keymaps.get(index).cloned() else {
      return;
    };
    match keyboard::set_console_keymap(&keymap, &self.console_keymaps) {
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

  fn apply_language(&mut self, index: usize) {
    let Some((locale, _, _)) = LANGUAGES.get(index) else {
      return;
    };
    self.lang = Lang::for_locale(locale);
    let value = serde_json::to_string(&self.lang.locale()).unwrap_or_else(|_| "\"en-US\"".into());
    let persisted = argvus_control_center_core::process::command("argvus-config")
      .args(["set", "/session/language", &value])
      .status()
      .is_ok_and(|status| status.success());
    let projected =
      persisted && argvus_control_center_core::config::reload_argvus_config_service().is_ok();
    if projected {
      self.success(tr(self.lang, "control_center.interface_language_applied").to_string());
    } else {
      self.fail("argvus-config could not persist the interface language");
    }
  }

  /// Enter/Space on a Mouse & Touchpad row: toggles flip, values step up.
  fn apply_input(&mut self, item: Item) {
    match item {
      Item::Ratbag(_) => self.input_cycle(item, 1),
      Item::Input(index @ (3 | 5 | 7..=11)) => {
        self.input_toggle_pending = Some(index);
        self.status = Some(Status::new(
          tr(self.lang, "control_center.input_applying").to_string(),
          StatusKind::Success,
        ));
      }
      Item::Input(_) => self.input_cycle(item, 1),
      _ => {}
    }
  }

  /// `←/→` (and `h/l`) on a Mouse & Touchpad row.
  pub(crate) fn input_cycle(&mut self, item: Item, direction: i8) {
    let selected = match item {
      Item::Ratbag(RatbagRow::Device) => return self.cycle_ratbag_device(direction),
      Item::Ratbag(RatbagRow::Profile) => {
        return self.queue_ratbag(crate::system::ratbag::Change::Profile(direction));
      }
      Item::Ratbag(RatbagRow::Dpi) => {
        return self.queue_ratbag(crate::system::ratbag::Change::Dpi(direction));
      }
      Item::Ratbag(RatbagRow::ReportRate) => {
        return self.queue_ratbag(crate::system::ratbag::Change::ReportRate(direction));
      }
      Item::Input(index) => index,
      _ => return,
    };
    let direction = direction.signum();
    let pending = self.input_pending.take();
    self.input_pending = Some(match pending {
      Some((pending_selected, pending_direction, _)) if pending_selected == selected => (
        selected,
        pending_direction.saturating_add(direction),
        std::time::Instant::now() + std::time::Duration::from_millis(100),
      ),
      _ => (
        selected,
        direction,
        std::time::Instant::now() + std::time::Duration::from_millis(100),
      ),
    });
    self.status = Some(Status::new(
      tr(self.lang, "control_center.input_applying").to_string(),
      StatusKind::Success,
    ));
  }

  /// Indexes of the shortcuts shown on the Keyboard Shortcuts page, in
  /// display order and narrowed by the search.
  pub(super) fn visible_keybindings(&self) -> Vec<usize> {
    let mut bindings: Vec<usize> = self
      .keybindings
      .iter()
      .enumerate()
      .filter(|(_, binding)| keybinding_is_cheatsheet_entry(binding))
      .filter(|(_, binding)| {
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
      .map(|(index, _)| index)
      .collect();
    bindings.sort_by_key(|index| {
      let binding = &self.keybindings[*index];
      (
        keybinding_section_order(binding),
        keybinding_category_order(&binding.category),
        binding.id.clone(),
      )
    });
    bindings
  }

  /// The shortcut under the cursor on the Keyboard Shortcuts page.
  pub(super) fn selected_keybinding(&self) -> Option<&keybindings::Binding> {
    match self.selected_item() {
      Some(Item::Keybinding(index)) => self.keybindings.get(index),
      _ => None,
    }
  }

  fn open_keybinding_edit(&mut self, index: usize) {
    let Some(id) = self
      .keybindings
      .get(index)
      .map(|binding| binding.id.clone())
    else {
      return;
    };
    self.keybinding_edit_id = Some(id);
    self.navigation.push(Page::KeybindingEdit);
    self.begin_keybinding_capture();
    self.keybinding_capturing = false;
    self.status = None;
  }

  /// "Disable" of the edit page: turns the edited shortcut off.
  fn disable_edited_keybinding(&mut self) {
    let Some(binding) = self.edited_binding() else {
      return;
    };
    let (id, keys) = (binding.id.clone(), binding.keys.clone());
    if let Err(error) = keybindings::save_override(&id, Some(keys), Some(false), &self.keybindings)
    {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    self.success(tr(self.lang, "control_center.keybindings_applied").to_string());
  }

  /// Space on the Keyboard Shortcuts list: enables or disables the
  /// shortcut under the cursor.
  pub(crate) fn toggle_selected_keybinding(&mut self) {
    let Some(binding) = self.selected_keybinding() else {
      return;
    };
    let (id, keys, enabled) = (binding.id.clone(), binding.keys.clone(), binding.enabled);
    if let Err(error) =
      keybindings::save_override(&id, Some(keys), Some(!enabled), &self.keybindings)
    {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    self.success(tr(self.lang, "control_center.keybindings_applied").to_string());
  }

  /// "Restore default" of the edit page: immediate, as before.
  fn restore_edited_keybinding(&mut self) {
    if let Some(id) = self.keybinding_edit_id.clone() {
      self.restore_keybinding(&id);
    }
  }

  /// Restores one shortcut to its ARGVUS default.
  pub(super) fn restore_keybinding(&mut self, id: &str) {
    if let Err(error) = keybindings::restore(id, &self.keybindings) {
      self.fail(error);
      return;
    }
    self.keybindings = keybindings::load();
    self.success(tr(self.lang, "control_center.keybindings_restored").to_string());
  }

  /// Reloads the saved rules and keeps the drafts whose rule still exists.
  pub(crate) fn reload_window_rules(&mut self) {
    self.window_rules = window_rules::load();
    let rules = &self.window_rules;
    self
      .window_rule_drafts
      .retain(|name, _| rules.iter().any(|rule| &rule.name == name));
  }

  /// Whether a rule differs from its saved state. Only drafts can differ.
  pub(crate) fn window_rule_changed(&self, rule: &window_rules::WindowRule) -> bool {
    self
      .window_rule_drafts
      .get(&rule.name)
      .is_some_and(|draft| draft != rule)
  }

  /// Changes the draft of a saved rule; nothing reaches the config until Apply.
  fn edit_window_rule(&mut self, index: usize, edit: impl FnOnce(&mut window_rules::WindowRule)) {
    let Some(loaded) = self.window_rules.get(index).cloned() else {
      return;
    };
    let draft = self
      .window_rule_drafts
      .entry(loaded.name.clone())
      .or_insert_with(|| loaded.clone());
    edit(draft);
  }

  fn adjust_window_rule(&mut self, index: usize, delta: i32) {
    self.edit_window_rule(index, |rule| {
      rule.workspace = window_rules::cycle_workspace(rule.workspace, delta);
    });
  }

  /// Opens the name editor of a saved rule, showing its draft name.
  fn open_window_rule_name(&mut self, index: usize) {
    let Some(loaded) = self.window_rules.get(index) else {
      return;
    };
    let draft = self.window_rule_drafts.get(&loaded.name).unwrap_or(loaded);
    self.admin.editor = Some(Editor::new(
      tr(self.lang, "control_center.window_rules_name").into(),
      draft.name.clone(),
      EditTarget::WindowRuleName(loaded.name.clone()),
      false,
    ));
  }

  fn open_window_rule_classes(&mut self, index: usize) {
    let Some(loaded) = self.window_rules.get(index) else {
      return;
    };
    let draft = self.window_rule_drafts.get(&loaded.name).unwrap_or(loaded);
    self.admin.editor = Some(Editor::new(
      tr(self.lang, "control_center.window_rules_classes").into(),
      draft.classes.join(", "),
      EditTarget::WindowRuleClasses(loaded.name.clone()),
      false,
    ));
  }

  fn open_new_window_rule_name(&mut self) {
    self.admin.editor = Some(Editor::new(
      tr(self.lang, "control_center.window_rules_name").into(),
      self.new_window_rule_name.clone(),
      EditTarget::NewWindowRuleName,
      false,
    ));
  }

  /// Saves the draft of one rule. A new name renames the rule.
  fn apply_window_rule(&mut self, index: usize) {
    let Some(loaded) = self.window_rules.get(index).cloned() else {
      return;
    };
    let Some(draft) = self.window_rule_drafts.get(&loaded.name).cloned() else {
      return;
    };
    if let Err(message) = self.check_window_rule_name(&draft.name, Some(loaded.name.as_str())) {
      self.fail(message);
      return;
    }
    match window_rules::save(Some(loaded.name.as_str()), &draft) {
      Ok(()) => {
        self.window_rule_drafts.remove(&loaded.name);
        self.reload_window_rules();
        self.success(tr(self.lang, "control_center.window_rules_applied").to_string());
      }
      Err(error) => self.fail(error),
    }
  }

  /// Adds a rule named by the typed name, or by the next free `rule-N` when empty.
  fn add_window_rule(&mut self) {
    let typed = self.new_window_rule_name.trim().to_owned();
    let name = if typed.is_empty() {
      window_rules::next_rule_name(&self.window_rules)
    } else {
      typed
    };
    if let Err(message) = self.check_window_rule_name(&name, None) {
      self.fail(message);
      return;
    }
    let rule = window_rules::WindowRule {
      name,
      workspace: 1,
      classes: Vec::new(),
    };
    match window_rules::save(None, &rule) {
      Ok(()) => {
        self.new_window_rule_name.clear();
        self.reload_window_rules();
        self.success(tr(self.lang, "control_center.window_rules_saved").to_string());
      }
      Err(error) => self.fail(error),
    }
  }

  /// Validates a rule name. `current` is the saved name of the rule being
  /// changed, which may keep its own name.
  pub(crate) fn check_window_rule_name(
    &self,
    name: &str,
    current: Option<&str>,
  ) -> Result<(), String> {
    if !window_rules::is_valid_name(name) {
      return Err(tr(self.lang, "control_center.window_rules_invalid_name").into());
    }
    let taken = self
      .window_rules
      .iter()
      .any(|rule| rule.name == name && Some(rule.name.as_str()) != current);
    if taken {
      return Err(tr(self.lang, "control_center.window_rules_name_in_use").into());
    }
    Ok(())
  }

  /// Opens the single-line editor for a new project path, or a new root.
  fn open_project_editor(&mut self, root: bool) {
    let title = if root {
      tr(self.lang, "control_center.projects_add_root")
    } else {
      tr(self.lang, "control_center.projects_add_path")
    };
    self.admin.editor = Some(Editor::new(
      title.into(),
      String::new(),
      EditTarget::ProjectPath(root),
      false,
    ));
  }

  fn open_new_snippet_name(&mut self) {
    self.admin.editor = Some(Editor::new(
      tr(self.lang, "control_center.snippets_name").into(),
      self.new_snippet_name.clone(),
      EditTarget::NewSnippetName,
      false,
    ));
  }

  fn open_new_snippet_content(&mut self) {
    self.admin.editor = Some(Editor::new(
      tr(self.lang, "control_center.snippets_content").into(),
      self.new_snippet_content.clone(),
      EditTarget::NewSnippetContent,
      false,
    ));
  }

  /// Opens the editor with a saved snippet's content; saving replaces it.
  fn open_snippet_content(&mut self, index: usize) {
    let Some(entry) = self.snippets.entries.get(index) else {
      return;
    };
    self.admin.editor = Some(Editor::new(
      tr(self.lang, "control_center.snippets_content").into(),
      entry.content.clone(),
      EditTarget::SnippetContent(entry.name.clone()),
      false,
    ));
  }

  fn add_snippet(&mut self) {
    let name = self.new_snippet_name.trim().to_owned();
    if name.is_empty() {
      self.fail(tr(self.lang, "control_center.snippets_name_required").to_string());
      return;
    }
    match snippets::add(&name, &self.new_snippet_content) {
      Ok(()) => {
        self.new_snippet_name.clear();
        self.new_snippet_content.clear();
        self.snippets = snippets::load();
        self.success(tr(self.lang, "control_center.snippets_saved").to_string());
      }
      Err(error) => self.fail(error),
    }
  }

  fn open_snippet_picker(&mut self) {
    match snippets::open_picker() {
      Ok(()) => self.success(tr(self.lang, "control_center.snippets_picker_opening").to_string()),
      Err(error) => self.fail(error),
    }
  }

  fn type_snippet(&mut self, index: usize) {
    match snippets::type_position(index + 1) {
      Ok(()) => self.success(tr(self.lang, "control_center.snippets_typing").to_string()),
      Err(error) => self.fail(error),
    }
  }
}
