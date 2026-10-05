use argvus_tui::menu::{Row, RowKind};
use crossterm::event::{Event, KeyCode, KeyEvent};

use super::*;
use crate::item::{Item, RatbagRow};

fn app(page: Page) -> App {
  let mut app = App::with_context(page, Lang::for_locale("en-US"), Theme::load());
  app.error_modal = None;
  app
}

fn press(app: &mut App, code: KeyCode) {
  crate::event::handle(app, Event::Key(KeyEvent::from(code)));
}

fn row_of(rows: &[Row<Item>], item: Item) -> &Row<Item> {
  rows
    .iter()
    .find(|row| row.id() == Some(&item))
    .unwrap_or_else(|| panic!("missing row {item:?}"))
}

#[test]
fn search_is_case_insensitive() {
  assert!(search_matches("nOtO", &["Noto Sans", "Regular"]));
  assert!(!search_matches("Roboto", &["Noto Sans"]));
}

#[test]
fn hardware_rows_follow_ratbag_capabilities() {
  let mut app = app(Page::MouseTouchpad);
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
  let hardware = rows
    .iter()
    .find(|row| row.label() == tr(app.lang, "control_center.input_hardware_mouse"))
    .expect("hardware section");
  assert!(hardware.is_section());
  assert_eq!(hardware.detail_text(), Some("Test mouse"));
  let dpi = row_of(&rows, Item::Ratbag(RatbagRow::Dpi));
  assert!(dpi.is_selectable());
  assert_eq!(dpi.kind(), RowKind::Value { step: Some(1) });
  assert!(row_of(&rows, Item::Ratbag(RatbagRow::ReportRate)).is_selectable());
  assert!(
    !row_of(&rows, Item::Ratbag(RatbagRow::Profile)).is_selectable(),
    "a single profile cannot be switched"
  );
  assert!(
    rows
      .iter()
      .all(|row| row.id() != Some(&Item::Ratbag(RatbagRow::Device)))
  );
}

#[test]
fn mouse_rows_are_typed_and_disabled_without_a_mouse() {
  let mut app = app(Page::MouseTouchpad);
  app.input_loading = false;
  app.input.devices.mouse = false;
  app.input.devices.touchpad = true;
  let rows = app.rows();
  assert_eq!(
    row_of(&rows, Item::Input(1)).kind(),
    RowKind::Value { step: Some(1) }
  );
  assert!(matches!(
    row_of(&rows, Item::Input(3)).kind(),
    RowKind::Toggle { .. }
  ));
  assert!(!row_of(&rows, Item::Input(1)).is_selectable());
  assert!(row_of(&rows, Item::Input(7)).is_selectable());
  assert_eq!(app.selected_item(), Some(Item::Input(7)));
}

#[test]
fn labels_cover_every_backend_category() {
  for category in Category::ORDER {
    assert!(!category_label(Lang::for_locale("en-US"), category).is_empty());
    assert!(!category_label(Lang::for_locale("pt-BR"), category).is_empty());
  }
}

#[test]
fn language_page_shows_status_lines_and_selectable_choices() {
  let app = app(Page::Language);
  let rows = app.rows();
  assert_eq!(rows.len(), 6);
  assert_eq!(rows[0].label(), "Current language");
  assert_eq!(rows[0].kind(), RowKind::Info);
  assert_eq!(rows[1].label(), "Regional locale");
  assert_eq!(rows[2].label(), "Encoding");
  assert_eq!(rows[3].kind(), RowKind::Separator);
  assert_eq!(rows[4].id(), Some(&Item::Language(0)));
  assert_eq!(rows[5].id(), Some(&Item::Language(1)));
  assert!(
    rows.iter().all(|row| !row.label().contains('🇺')),
    "no flag emoji"
  );
  assert_eq!(
    rows[4].kind(),
    RowKind::Choice { current: true },
    "English is current"
  );
  assert_eq!(app.selected_item(), Some(Item::Language(0)));
}

#[test]
fn fonts_dashboard_has_sections_icons_status_and_danger_zone() {
  let app = app(Page::Fonts);
  let rows = app.rows();
  let targets: Vec<&Row<Item>> = rows
    .iter()
    .filter(|row| matches!(row.id(), Some(Item::FontTarget(_))))
    .collect();
  assert_eq!(targets.len(), FontTarget::ALL.len());
  assert_eq!(targets[0].icon_glyph(), Some(argvus_tui::icons::TASKBAR));
  assert!(targets.iter().all(|row| row.detail_text().is_some()));
  let settings = rows
    .iter()
    .filter(|row| matches!(row.id(), Some(Item::FontSetting(_))))
    .count();
  assert_eq!(settings, SettingKind::ALL.len());
  assert_eq!(rows.iter().filter(|row| row.is_section()).count(), 3);
  let last = rows.last().unwrap();
  assert_eq!(last.id(), Some(&Item::ResetDefaults));
  assert_eq!(last.kind(), RowKind::Destructive);
}

#[test]
fn fonts_dashboard_icons_are_distinct() {
  let mut seen = std::collections::HashSet::new();
  for target in FontTarget::ALL {
    let icon = font_target_icon(target);
    assert!(seen.insert(icon), "duplicate icon {icon} for {target:?}");
  }
  for setting in SettingKind::ALL {
    let icon = setting_icon(setting);
    assert!(seen.insert(icon), "duplicate icon {icon} for {setting:?}");
  }
}

#[test]
fn default_apps_dashboard_uses_icons_and_status() {
  let app = app(Page::DefaultApps);
  let rows = app.rows();
  assert_eq!(rows.len(), Category::ORDER.len() + 2);
  assert!(rows[0].label().contains("Terminal"), "{}", rows[0].label());
  assert_eq!(rows[0].icon_glyph(), Some(argvus_tui::icons::TERMINAL));
  assert_eq!(rows[0].kind(), RowKind::Submenu);
  for row in &rows[..Category::ORDER.len()] {
    assert!(row.detail_text().is_some());
  }
  assert_eq!(rows.last().unwrap().id(), Some(&Item::ResetDefaults));
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
fn reset_row_and_r_open_the_same_confirmation() {
  let mut app = app(Page::DefaultApps);
  press(&mut app, KeyCode::End);
  assert_eq!(app.selected_item(), Some(Item::ResetDefaults));
  press(&mut app, KeyCode::Enter);
  assert!(matches!(app.confirm, Some(PendingAction::ResetApps)));
  press(&mut app, KeyCode::Esc);
  assert!(app.confirm.is_none());
  press(&mut app, KeyCode::Char('r'));
  assert!(matches!(app.confirm, Some(PendingAction::ResetApps)));
}

#[test]
fn system_dashboard_uses_typed_rows_and_live_state() {
  let app = app(Page::System);
  let rows = app.rows();
  assert_eq!(rows.len(), 4);
  assert_eq!(rows[0].id(), Some(&Item::Hostname));
  assert_eq!(rows[0].kind(), RowKind::Value { step: None });
  assert_eq!(rows[0].icon_glyph(), Some(argvus_tui::icons::MONITOR));
  assert_eq!(rows[1].label(), "Users");
  assert_eq!(rows[1].detail_text(), Some("N/A"));
  assert_eq!(rows[2].id(), Some(&Item::Groups));
  assert!(!rows[2].label().ends_with(':'));
  assert_eq!(rows[2].detail_text(), Some("N/A"));
  assert!(matches!(rows[3].kind(), RowKind::Toggle { .. }));
  assert!(rows[3].label().contains("Do Not Disturb"));
}

#[test]
fn hostname_row_opens_the_editor_directly() {
  let mut app = app(Page::System);
  app.hostname = "current-machine".into();
  press(&mut app, KeyCode::Enter);
  assert!(app.hostname_editing);
  assert_eq!(app.hostname_input, "current-machine");
  assert_eq!(app.page(), Page::System);
}

#[test]
fn hostname_page_row_keeps_current_value_while_editing() {
  let mut app = app(Page::Hostname);
  app.hostname = "current-machine".into();
  app.hostname_editing = true;
  app.hostname_input = "draft-name".into();
  let rows = app.rows();
  assert_eq!(rows.len(), 1);
  assert!(rows[0].label().contains("Current hostname"));
  assert_eq!(rows[0].detail_text(), Some("current-machine"));
}

#[test]
fn system_dashboard_counts_match_loaded_accounts() {
  let mut app = app(Page::System);
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
  assert_eq!(rows[1].detail_text(), Some("1"));
  assert_eq!(rows[2].detail_text(), Some("1"));
  assert!(rows.iter().all(Row::is_selectable));
}

#[test]
fn font_size_is_a_value_row_with_arrows_and_shortcuts() {
  let mut app = app(Page::Fonts);
  app.navigation.push(Page::FontSelector(FontTarget::Apps));
  app.pending_size = 13;
  let rows = app.rows();
  assert_eq!(rows[0].id(), Some(&Item::FontSize));
  assert_eq!(rows[0].kind(), RowKind::Value { step: Some(1) });
  assert!(
    app
      .navigation
      .current_mut()
      .menu
      .select(&rows, &Item::FontSize)
  );
  press(&mut app, KeyCode::Right);
  assert_eq!(app.pending_size, 14);
  press(&mut app, KeyCode::Left);
  press(&mut app, KeyCode::Char('-'));
  assert_eq!(app.pending_size, 12);
  assert_eq!(app.page(), Page::FontSelector(FontTarget::Apps));
  press(&mut app, KeyCode::Enter);
  assert!(app.admin.editor.is_some(), "Enter opens the size prompt");
}

#[test]
fn system_locales_toggle_with_enter_and_apply_from_the_last_row() {
  let mut app = app(Page::SystemLocales);
  app.locale_gen_entries = vec![
    locale::LocaleEntry {
      locale: "en_US.UTF-8".into(),
      encoding: "UTF-8".into(),
      enabled: true,
      line_index: 0,
    },
    locale::LocaleEntry {
      locale: "pt_BR.UTF-8".into(),
      encoding: "UTF-8".into(),
      enabled: false,
      line_index: 1,
    },
  ];
  app.selected_locales = ["en_US.UTF-8 UTF-8".to_string()].into();
  app.navigation.current_mut().menu = argvus_tui::menu::MenuState::default();
  let apply = |app: &App| {
    app
      .rows()
      .into_iter()
      .find(|row| row.id() == Some(&Item::ApplyLocales))
      .unwrap()
      .is_selectable()
  };
  assert!(!apply(&app), "Apply is dimmed without changes");
  press(&mut app, KeyCode::Down);
  assert_eq!(app.selected_item(), Some(Item::LocaleGen(1)));
  press(&mut app, KeyCode::Enter);
  assert!(app.selected_locales.contains("pt_BR.UTF-8 UTF-8"));
  assert!(app.confirm.is_none(), "Enter toggles instead of applying");
  press(&mut app, KeyCode::End);
  assert_eq!(
    app.selected_item(),
    Some(Item::ApplyLocales),
    "Apply is enabled"
  );
  press(&mut app, KeyCode::Enter);
  assert!(matches!(
    app.confirm,
    Some(PendingAction::ApplySystemLocales)
  ));
}

#[test]
fn r_on_shortcuts_restores_the_selected_shortcut_after_confirmation() {
  let mut app = app(Page::Keybindings);
  let Some(first) = app.visible_keybindings().first().copied() else {
    return; // no keybinding catalog in this environment
  };
  let rows = app.rows();
  assert!(
    app
      .navigation
      .current_mut()
      .menu
      .select(&rows, &Item::Keybinding(first))
  );
  press(&mut app, KeyCode::Char('r'));
  let id = app.keybindings[first].id.clone();
  assert!(matches!(
    &app.confirm,
    Some(PendingAction::RestoreKeybinding { id: pending, leave: false }) if *pending == id
  ));
  assert_eq!(
    app.rows().last().and_then(|row| row.id().copied()),
    Some(Item::RestoreAllShortcuts)
  );
}

#[test]
fn keyboard_layout_footer_lists_enter_and_space() {
  let mut app = app(Page::KeyboardLayout);
  app.keyboard_layouts = vec![keyboard::Layout {
    code: "us".into(),
    description: "English (US)".into(),
  }];
  app.navigation.current_mut().menu = argvus_tui::menu::MenuState::default();
  let footer = app.footer();
  assert!(footer.contains("Enter "), "{footer}");
  assert!(footer.contains("Space "), "{footer}");
  assert!(!footer.contains("Enter/Space"), "{footer}");
}

#[test]
fn shortcut_conflict_is_a_confirmation_that_esc_cancels() {
  let mut app = app(Page::Keybindings);
  app.keybinding_conflict = Some(("a".into(), "SUPER + Q".into(), vec!["b".into()]));
  let (title, message, confirm, danger) = app.confirm_dialog().unwrap();
  assert_eq!(
    title,
    tr(app.lang, "control_center.keybindings_conflict_title")
  );
  assert!(message.contains("SUPER + Q"));
  assert_eq!(confirm, tr(app.lang, "control_center.replace"));
  assert!(!danger);
  assert!(app.footer().contains("r "), "{}", app.footer());
  press(&mut app, KeyCode::Enter);
  assert!(!app.has_keybinding_conflict(), "Enter on Cancel cancels");
  app.keybinding_conflict = Some(("a".into(), "SUPER + Q".into(), vec!["b".into()]));
  press(&mut app, KeyCode::Esc);
  assert!(!app.has_keybinding_conflict());
  assert_eq!(app.page(), Page::Keybindings);
}

#[test]
fn task_window_scrolls_and_closes() {
  let mut app = app(Page::Main);
  let live = LiveProcess::new();
  for _ in 0..40 {
    live.push_line("line");
  }
  app.task_live = Some(live.clone());
  app.task_open = true;
  app.task_scroll = 0;
  press(&mut app, KeyCode::Down);
  assert_eq!(app.task_scroll, 1);
  press(&mut app, KeyCode::Up);
  assert_eq!(app.task_scroll, 0);
  press(&mut app, KeyCode::PageDown);
  assert_eq!(app.task_scroll, 10);
  press(&mut app, KeyCode::Home);
  assert_eq!(app.task_scroll, 0);
  press(&mut app, KeyCode::End);
  assert_eq!(
    app.task_scroll as usize,
    40usize.saturating_sub(TASK_CONTENT_HEIGHT)
  );
  press(&mut app, KeyCode::Esc);
  assert!(!app.task_open);
  assert_eq!(app.task_scroll, 0);
}

#[test]
fn task_bottom_offset_is_zero_for_short_output() {
  let mut app = app(Page::Main);
  let live = LiveProcess::new();
  live.push_line("done.");
  app.task_live = Some(live);
  app.task_open = true;
  press(&mut app, KeyCode::End);
  assert_eq!(app.task_scroll, 0);
  assert_eq!(task_bottom_offset("done."), 0);
}

#[test]
fn empty_poll_returns_false() {
  let mut app = app(Page::Main);
  assert!(!app.poll());
}
