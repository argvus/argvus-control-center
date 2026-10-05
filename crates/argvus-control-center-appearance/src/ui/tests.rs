use super::*;
use crate::model::{
  ControlPanelCard, TaskbarPosition, TaskbarUtilityWidget, WallpaperCollection, WallpaperMode,
  WidgetTelemetryBlock,
};
use argvus_tui::icons;
use ratatui::{Terminal, backend::TestBackend};

fn app(page: AppearancePage) -> AppearanceApp {
  AppearanceApp {
    page,
    loaded: true,
    ..AppearanceApp::new(Lang::for_locale("en-US"), Theme::load())
  }
}

/// A page whose draft was loaded the way `go` loads it.
fn draft_app(page: AppearancePage) -> AppearanceApp {
  let mut application = app(page);
  let surface = AppearanceApp::surface_for_page(page).expect("draft page");
  application.surface_draft = Some(application.make_surface_draft(surface));
  application
}

fn labels(application: &AppearanceApp) -> Vec<String> {
  application
    .rows()
    .iter()
    .map(|row| row.label().to_owned())
    .collect()
}

/// Moves the cursor to `item`, failing when the row is not selectable.
fn select(application: &mut AppearanceApp, item: Item) {
  let rows = application.rows();
  application.menu.normalize(&rows);
  assert!(
    application.menu.select(&rows, &item),
    "{item:?} is not selectable on {:?}",
    application.page
  );
}

fn selected(application: &AppearanceApp) -> Option<Item> {
  let rows = application.rows();
  let mut menu = application.menu;
  menu.normalize(&rows);
  menu.selected_id(&rows)
}

fn render(application: &mut AppearanceApp, width: u16, height: u16) -> Vec<String> {
  let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
  terminal.draw(|frame| application.draw(frame)).unwrap();
  terminal
    .backend()
    .buffer()
    .content
    .chunks(usize::from(width))
    .map(|line| line.iter().map(|cell| cell.symbol()).collect())
    .collect()
}

fn line_with<'screen>(screen: &'screen [String], needle: &str) -> &'screen str {
  screen
    .iter()
    .find(|line| line.contains(needle))
    .unwrap_or_else(|| panic!("{needle} missing: {screen:#?}"))
}

#[test]
fn home_has_categories_and_spacing_is_nested() {
  assert_eq!(app(AppearancePage::Home).rows().len(), 11);
  assert_eq!(app(AppearancePage::SpacesBordersPosition).rows().len(), 5);
  assert_eq!(app(AppearancePage::Taskbar).rows().len(), 4);
  assert_eq!(
    app(AppearancePage::TaskbarIcons).rows().len(),
    2 + TaskbarUtilityWidget::ALL.len() + 1
  );
  assert_eq!(app(AppearancePage::TaskbarDate).rows().len(), 1);
  assert_eq!(app(AppearancePage::TaskbarDateFormat).rows().len(), 4);
  assert_eq!(app(AppearancePage::TaskbarTime).rows().len(), 2);
  assert_eq!(app(AppearancePage::TaskbarTimeFormat).rows().len(), 2);
  assert_eq!(app(AppearancePage::WidgetTelemetry).rows().len(), 3);
  assert_eq!(app(AppearancePage::ControlPanel).rows().len(), 3);
  assert_eq!(app(AppearancePage::Effects).rows().len(), 3);
  assert_eq!(app(AppearancePage::Blur).rows().len(), 1);
  assert_eq!(app(AppearancePage::Terminal).rows().len(), 2);
}

#[test]
fn rows_have_the_kind_of_what_they_do() {
  let home = app(AppearancePage::Home).rows();
  assert!(home.iter().all(|row| row.kind() == RowKind::Submenu));

  let effects = app(AppearancePage::Effects).rows();
  assert_eq!(effects[0].kind(), RowKind::Toggle { on: true });
  assert_eq!(effects[2].kind(), RowKind::Submenu);
  assert_eq!(effects[2].detail_text(), Some("50%"));

  let position = app(AppearancePage::TaskbarPosition).rows();
  assert_eq!(position[0].kind(), RowKind::Choice { current: true });
  assert_eq!(position[1].kind(), RowKind::Choice { current: false });

  let spaces = app(AppearancePage::TaskbarSpaces).rows();
  assert!(
    spaces
      .iter()
      .all(|row| row.kind() == RowKind::Value { step: None })
  );

  let editor = app(AppearancePage::Blur).rows();
  assert_eq!(editor[0].kind(), RowKind::Value { step: Some(5) });
}

#[test]
fn every_navigable_row_owns_a_semantic_icon() {
  let home = app(AppearancePage::Home).rows();
  let glyphs: Vec<_> = home.iter().map(|row| row.icon_glyph()).collect();
  assert_eq!(glyphs[0], Some(icons::PALETTE));
  assert_eq!(glyphs[1], Some(icons::ACCENT));
  assert_eq!(
    glyphs[4],
    Some(icons::TASKBAR),
    "Taskbar is no longer an HDD"
  );
  assert_eq!(glyphs[5], Some(icons::EFFECT));
  let unique: std::collections::HashSet<_> = glyphs.iter().collect();
  assert_eq!(unique.len(), glyphs.len(), "Home siblings repeat an icon");
  assert!(
    !glyphs
      .iter()
      .any(|glyph| matches!(*glyph, Some(icons::STORAGE | icons::INFO | icons::SUCCESS))),
    "wildcard icons are gone"
  );

  let mode = app(AppearancePage::Mode).rows();
  assert_eq!(mode[0].icon_glyph(), Some(icons::WINDOW_STICKY));
  assert_eq!(mode[1].icon_glyph(), Some(icons::WINDOW_FLOAT));

  let position = app(AppearancePage::TaskbarPosition).rows();
  assert_eq!(position[0].icon_glyph(), Some(icons::ARROW_UP));
  assert_eq!(position[1].icon_glyph(), Some(icons::ARROW_DOWN));
}

#[test]
fn data_lists_carry_no_icon_so_the_column_disappears() {
  let mut application = app(AppearancePage::WallpaperItems {
    collection: WallpaperCollection::Abstract,
    mode: WallpaperMode::Dark,
  });
  application.state.wallpapers = vec![crate::WallpaperEntry {
    path: "abstract/dark/one.jxl".into(),
    collection: WallpaperCollection::Abstract,
    mode: WallpaperMode::Dark,
  }];
  assert!(
    application
      .rows()
      .iter()
      .all(|row| row.icon_glyph().is_none())
  );
}

#[test]
fn mode_page_lists_sticky_and_float_independent_of_theme() {
  let rows = labels(&app(AppearancePage::Mode));
  assert_eq!(rows.len(), 2);
  assert!(rows[0].contains("Sticky"));
  assert!(rows[1].contains("Float"));

  let mut ui = app(AppearancePage::Home);
  assert_eq!(selected(&ui), Some(Item::Themes));
  select(&mut ui, Item::Mode);
  ui.handle(KeyCode::Enter);
  assert_eq!(ui.page, AppearancePage::Mode);
}

#[test]
fn take_theme_dirty_clears_after_first_read() {
  let mut ui = app(AppearancePage::Home);
  assert!(!ui.take_theme_dirty());
  ui.theme_dirty = true;
  assert!(ui.take_theme_dirty());
  assert!(!ui.take_theme_dirty());
}

#[test]
fn set_theme_replaces_the_semantic_palette() {
  let mut ui = app(AppearancePage::Home);
  let mut theme = Theme::load();
  theme.name = "marker-theme".into();
  ui.set_theme(&theme);
  assert_eq!(ui.theme.name, "marker-theme");
}

#[test]
fn surface_pages_keep_settings_nested_and_values_bounded() {
  let widget_sessions = draft_app(AppearancePage::SurfaceSection {
    surface: EffectSurface::WidgetTelemetry,
    section: SurfaceSection::Sessions,
  });
  assert_eq!(
    widget_sessions.rows().len(),
    WidgetTelemetryBlock::ALL.len()
  );
  let control_sessions = draft_app(AppearancePage::SurfaceSection {
    surface: EffectSurface::ControlPanel,
    section: SurfaceSection::Sessions,
  });
  assert_eq!(control_sessions.rows().len(), ControlPanelCard::ALL.len());

  let mut application = draft_app(AppearancePage::SurfaceSection {
    surface: EffectSurface::Taskbar,
    section: SurfaceSection::Transparency,
  });
  application.surface_draft.as_mut().unwrap().transparency = 100;
  select(&mut application, Item::SectionValue);
  application.handle(KeyCode::Char('+'));
  application.handle(KeyCode::Right);
  assert_eq!(application.surface_draft().unwrap().transparency, 100);
  application.surface_draft.as_mut().unwrap().transparency = 0;
  application.handle(KeyCode::Char('-'));
  application.handle(KeyCode::Left);
  assert_eq!(application.surface_draft().unwrap().transparency, 0);
}

#[test]
fn control_panel_sessions_toggle_the_card_on_the_row() {
  // Regression: the cursor used to index `ControlPanelCard::ALL` while the
  // page lists only available cards, so with a hidden card the toggle hit
  // the card below the cursor.
  let mut application = draft_app(AppearancePage::SurfaceSection {
    surface: EffectSurface::ControlPanel,
    section: SurfaceSection::Sessions,
  });
  let draft = application.surface_draft.as_mut().unwrap();
  draft
    .control_panel_cards
    .set_available(ControlPanelCard::User, false);
  let target = ControlPanelCard::Notifications;
  let before = draft.control_panel_cards.enabled(target);
  let neighbour = draft
    .control_panel_cards
    .enabled(ControlPanelCard::Calendar);

  let rows = application.rows();
  assert_eq!(rows.len(), ControlPanelCard::ALL.len() - 1);
  assert_eq!(rows[0].id(), Some(&Item::PanelCard(target)));
  application.handle(KeyCode::Enter);

  let cards = &application.surface_draft().unwrap().control_panel_cards;
  assert_eq!(cards.enabled(target), !before);
  assert_eq!(cards.enabled(ControlPanelCard::Calendar), neighbour);
}

#[test]
fn theme_rows_use_friendly_official_names() {
  let rows = labels(&app(AppearancePage::Themes));
  // Official, Custom, export profile, import profile.
  assert_eq!(rows.len(), 4);
  assert!(rows[0].contains("Official"));
  assert!(rows[1].contains("Custom"));
  assert!(rows[2].contains("Export theme profile"));
  assert!(rows[3].contains("Import theme profile"));
}

#[test]
fn official_theme_rows_expose_both_categories() {
  let rows = app(AppearancePage::OfficialThemes).rows();
  assert_eq!(
    rows.iter().map(|row| row.label()).collect::<Vec<_>>(),
    vec!["Dark", "Light"]
  );
  assert!(rows.iter().all(|row| row.kind() == RowKind::Submenu));
}

#[test]
fn wallpaper_rows_use_nerd_font_icon_and_nested_categories() {
  let rows = app(AppearancePage::Wallpapers).rows();
  assert_eq!(rows.len(), 3);
  assert!(rows[0].label().contains("Choose image from HOME"));
  assert_eq!(rows[0].kind(), RowKind::Action);
  assert_eq!(rows[0].icon_glyph(), Some(icons::FOLDER));
  assert!(!rows[0].label().contains("📂"));
  assert_eq!(
    rows[1].id(),
    Some(&Item::Collection(WallpaperCollection::Abstract))
  );
  assert_eq!(
    rows[2].id(),
    Some(&Item::Collection(WallpaperCollection::Landscape))
  );

  let modes = app(AppearancePage::WallpaperModes {
    collection: WallpaperCollection::Abstract,
  })
  .rows();
  assert_eq!(
    modes[0].id(),
    Some(&Item::WallpaperMode(WallpaperMode::Dark))
  );
  assert_eq!(
    modes[1].id(),
    Some(&Item::WallpaperMode(WallpaperMode::Light))
  );
}

#[test]
fn wallpaper_navigation_returns_from_items_to_collection() {
  let mut application = app(AppearancePage::Wallpapers);
  select(
    &mut application,
    Item::Collection(WallpaperCollection::Abstract),
  );
  application.handle(KeyCode::Enter);
  application.job = None;
  assert_eq!(
    application.page,
    AppearancePage::WallpaperModes {
      collection: WallpaperCollection::Abstract
    }
  );
  application.handle(KeyCode::Right);
  application.job = None;
  assert_eq!(
    application.page,
    AppearancePage::WallpaperItems {
      collection: WallpaperCollection::Abstract,
      mode: WallpaperMode::Dark
    }
  );
  application.handle(KeyCode::Left);
  assert_eq!(
    application.page,
    AppearancePage::WallpaperModes {
      collection: WallpaperCollection::Abstract
    }
  );
  application.handle(KeyCode::Esc);
  assert_eq!(application.page, AppearancePage::Wallpapers);
}

#[test]
fn wallpaper_items_filter_by_collection_and_mode() {
  let mut application = app(AppearancePage::WallpaperItems {
    collection: WallpaperCollection::Landscape,
    mode: WallpaperMode::Light,
  });
  application.state.wallpapers = vec![
    crate::WallpaperEntry {
      path: "abstract/dark/gruvbox-abstract-dark.jxl".into(),
      collection: WallpaperCollection::Abstract,
      mode: WallpaperMode::Dark,
    },
    crate::WallpaperEntry {
      path: "landscape/light/gruvbox-landscape-light.jxl".into(),
      collection: WallpaperCollection::Landscape,
      mode: WallpaperMode::Light,
    },
  ];
  application.state.wallpaper_active = Some("landscape/light/gruvbox-landscape-light.jxl".into());
  let rows = application.rows();
  assert_eq!(labels(&application), vec!["gruvbox-landscape-light.jxl"]);
  assert_eq!(
    rows[0].id(),
    Some(&Item::WallpaperFile(1)),
    "keeps its identity"
  );
  assert_eq!(rows[0].kind(), RowKind::Choice { current: true });
}

// TODO (Fase 3): Refactor to test with dynamic theme discovery
// Currently themes are discovered dynamically from /usr/share/argvus/appearance/themes.d/
// Test needs a temporary directory with sample theme.toml files to verify grouping
#[test]
fn theme_families_are_grouped_by_identifier_category() {
  let dark_rows = app(AppearancePage::ThemeFamilies {
    category: ThemeCategory::Dark,
  })
  .rows();
  let light_rows = app(AppearancePage::ThemeFamilies {
    category: ThemeCategory::Light,
  })
  .rows();
  // Without drop-in packages installed, only built-in themes appear
  assert_eq!(dark_rows.len(), 1); // argvus-dark
  assert_eq!(light_rows.len(), 1); // argvus-light
}

#[test]
fn builtin_themes_always_present() {
  let report = argvus_theme::discovery::discover_themes(std::path::Path::new("/tmp"));
  assert!(report.themes.iter().any(|t| t.id == "argvus-dark"));
  assert!(report.themes.iter().any(|t| t.id == "argvus-light"));
  // Verify categories are correct
  let dark = report
    .themes
    .iter()
    .find(|t| t.id == "argvus-dark")
    .unwrap();
  assert_eq!(dark.category, argvus_theme::discovery::ThemeCategory::Dark);
  let light = report
    .themes
    .iter()
    .find(|t| t.id == "argvus-light")
    .unwrap();
  assert_eq!(
    light.category,
    argvus_theme::discovery::ThemeCategory::Light
  );
}

#[test]
fn home_rows_follow_the_global_icon_setting() {
  AppConfig::set_session_icons(true);
  let screen = render(&mut app(AppearancePage::Home), 80, 24);
  assert!(line_with(&screen, "Theme").contains(icons::PALETTE));
  assert!(line_with(&screen, "Taskbar").contains(icons::TASKBAR));

  AppConfig::set_session_icons(false);
  let screen = render(&mut app(AppearancePage::Home), 80, 24);
  assert!(!line_with(&screen, "Theme").contains(icons::PALETTE));
  assert!(!line_with(&screen, "Taskbar").contains(icons::TASKBAR));
  AppConfig::set_session_icons(true);
}

#[test]
fn home_renders_one_vertical_list_at_80_columns() {
  let screen = render(&mut app(AppearancePage::Home), 80, 24);
  let theme = line_with(&screen, "Theme");
  assert!(
    theme.contains("> "),
    "first row holds the cursor: {theme:?}"
  );
  assert!(theme.contains('›'), "submenus show the marker: {theme:?}");
  assert!(
    !screen.iter().any(|line| line.contains("[ ")),
    "no button labels"
  );
}

#[test]
fn selection_bounds_follow_each_page() {
  let mut application = app(AppearancePage::WindowSpaces);
  application.handle(KeyCode::End);
  assert_eq!(
    selected(&application),
    Some(Item::Spacing(PromptGoal::GapsOutBottom))
  );
  application.handle(KeyCode::Down);
  assert_eq!(
    selected(&application),
    Some(Item::Spacing(PromptGoal::GapsOutBottom))
  );
}

#[test]
fn disabled_rounding_is_skipped_and_does_not_open_prompt() {
  let mut application = app(AppearancePage::GeneralBorders);
  assert!(!application.state.rounded);
  let rows = application.rows();
  assert!(!rows[1].is_selectable(), "Rounding is disabled");
  application.handle(KeyCode::End);
  assert_eq!(selected(&application), Some(Item::Rounded));
  application.activate(Item::Rounding);
  assert!(application.prompt_back.is_none());

  application.state.rounded = true;
  select(&mut application, Item::Rounding);
  application.handle(KeyCode::Enter);
  assert_eq!(
    application.page,
    AppearancePage::Prompt {
      goal: PromptGoal::Rounding
    }
  );
}

#[test]
fn space_keeps_activating_actions_submenus_choices_and_editors() {
  // Submenu: Space opens the page, as Enter.
  let mut home = app(AppearancePage::Home);
  select(&mut home, Item::Effects);
  home.handle(KeyCode::Char(' '));
  assert_eq!(home.page, AppearancePage::Effects);

  // Choice: Space applies the option immediately, as Enter.
  let mut position = app(AppearancePage::TaskbarPosition);
  select(&mut position, Item::Position(TaskbarPosition::Bottom));
  position.handle(KeyCode::Char(' '));
  assert!(position.action.is_some());

  // Action: Space opens the export prompt.
  let mut themes = app(AppearancePage::Themes);
  select(&mut themes, Item::ExportTheme);
  themes.handle(KeyCode::Char(' '));
  assert_eq!(
    themes.page,
    AppearancePage::Prompt {
      goal: PromptGoal::ExportProfile
    }
  );

  // Value without a step: Space opens its editor.
  let mut spaces = app(AppearancePage::TaskbarSpaces);
  select(&mut spaces, Item::Spacing(PromptGoal::WaybarLeft));
  spaces.handle(KeyCode::Char(' '));
  assert_eq!(
    spaces.page,
    AppearancePage::Prompt {
      goal: PromptGoal::WaybarLeft
    }
  );

  // Toggle: Space flips it (shared menu behavior).
  let mut time = draft_app(AppearancePage::TaskbarTime);
  let before = time.surface_draft().unwrap().time_seconds_enabled;
  time.handle(KeyCode::Char(' '));
  assert_eq!(time.surface_draft().unwrap().time_seconds_enabled, !before);
}

#[test]
fn value_rows_adjust_with_arrows_and_footer_says_esc_goes_back() {
  let mut editor = app(AppearancePage::Blur);
  let rows = editor.rows();
  editor.menu.normalize(&rows);
  let footer = editor.hints(&rows);
  assert!(footer.contains("←/→ "), "{footer}");
  assert!(footer.contains("Esc "), "{footer}");
  assert!(!footer.contains("←/Esc"), "← adjusts here: {footer}");

  editor.handle(KeyCode::Right);
  assert_eq!(editor.effect_draft, Some(55));
  editor.handle(KeyCode::Left);
  editor.handle(KeyCode::Left);
  assert_eq!(editor.effect_draft, Some(45));
  assert_eq!(
    editor.page,
    AppearancePage::Blur,
    "← adjusted, did not leave"
  );
  editor.handle(KeyCode::Char('h'));
  editor.handle(KeyCode::Char('l'));
  editor.handle(KeyCode::Char('l'));
  assert_eq!(editor.effect_draft, Some(50));
}

#[test]
fn theme_shortcuts_are_kept_and_shown_in_the_footer() {
  let mut custom = app(AppearancePage::CustomThemes);
  custom.state.custom_themes = vec![CustomTheme {
    id: "mine".into(),
    name: "Mine".into(),
    base_theme: "argvus-dark".into(),
    profile_path: "/tmp/mine".into(),
    wallpaper_path: None,
  }];
  let rows = custom.rows();
  custom.menu.normalize(&rows);
  let footer = custom.hints(&rows);
  for key in ["d ", "e ", "i "] {
    assert!(footer.contains(key), "{key} missing: {footer}");
  }
  custom.handle(KeyCode::Char('d'));
  assert_eq!(custom.page, AppearancePage::ThemeDeleteConfirm);
  assert_eq!(custom.delete_theme.as_ref().unwrap().id, "mine");

  let mut themes = app(AppearancePage::Themes);
  themes.handle(KeyCode::Char('i'));
  assert_eq!(themes.page, AppearancePage::ThemeImport);
}

#[test]
fn control_panel_apply_is_reached_through_tab_actions() {
  let mut application = draft_app(AppearancePage::ControlPanel);
  assert!(!labels(&application).iter().any(|row| row.contains("Apply")));
  application.handle(KeyCode::Tab);
  assert!(application.on_buttons);
  assert_eq!(application.button_selected, 0);
  application.handle(KeyCode::BackTab);
  assert!(!application.on_buttons);
}

#[test]
fn surface_value_navigation_keeps_vertical_focus_and_exposes_apply_actions() {
  for surface in EffectSurface::ALL {
    for section in [SurfaceSection::Transparency, SurfaceSection::Blur] {
      let mut application = draft_app(AppearancePage::SurfaceSection { surface, section });
      assert_eq!(application.buttons().len(), 1);

      application.handle(KeyCode::Down);
      assert_eq!(selected(&application), Some(Item::SectionValue));
      application.handle(KeyCode::Up);
      assert_eq!(selected(&application), Some(Item::SectionEnabled));

      select(&mut application, Item::SectionValue);
      let value = |application: &AppearanceApp| {
        let draft = application.surface_draft().unwrap();
        if section == SurfaceSection::Transparency {
          draft.transparency
        } else {
          draft.blur
        }
      };
      let initial = value(&application);
      application.handle(KeyCode::Char('+'));
      assert_eq!(selected(&application), Some(Item::SectionValue));
      assert_eq!(value(&application), (initial + 5).min(100));

      application.handle(KeyCode::Tab);
      assert!(application.on_buttons);
      application.handle(KeyCode::Enter);
      assert!(application.action.is_some());
    }
  }
}

#[test]
fn taskbar_icons_date_time_pages_toggle_and_expose_apply_actions() {
  let mut icons_page = draft_app(AppearancePage::TaskbarIcons);
  let draft = |application: &AppearanceApp| application.surface_draft().unwrap().clone();

  let before = draft(&icons_page);
  select(&mut icons_page, Item::AudioPlayer);
  icons_page.handle(KeyCode::Enter);
  select(&mut icons_page, Item::TaskbarLauncher);
  icons_page.handle(KeyCode::Enter);
  let widget = TaskbarUtilityWidget::GpuTemperature;
  select(&mut icons_page, Item::UtilityWidget(widget));
  icons_page.handle(KeyCode::Enter);
  let after = draft(&icons_page);
  assert_eq!(after.audio_player_enabled, !before.audio_player_enabled);
  assert_eq!(after.launcher_enabled, !before.launcher_enabled);
  assert_eq!(
    after.utility_widgets.enabled(widget),
    !before.utility_widgets.enabled(widget)
  );

  // "Utilities ›" opens the shared `SurfaceSection{Taskbar, UtilityIcons}`
  // page (Always expanded / Expand on hover).
  select(&mut icons_page, Item::Utilities);
  icons_page.handle(KeyCode::Enter);
  assert_eq!(
    icons_page.page,
    AppearancePage::SurfaceSection {
      surface: EffectSurface::Taskbar,
      section: SurfaceSection::UtilityIcons,
    }
  );

  let mut utility_group = draft_app(AppearancePage::SurfaceSection {
    surface: EffectSurface::Taskbar,
    section: SurfaceSection::UtilityIcons,
  });
  for mode in [
    TaskbarUtilityGroupMode::AlwaysExpanded,
    TaskbarUtilityGroupMode::Auto,
  ] {
    select(&mut utility_group, Item::UtilityGroup(mode));
    utility_group.handle(KeyCode::Enter);
    assert_eq!(utility_group.surface_draft().unwrap().utility_group, mode);
  }

  let mut date_format = draft_app(AppearancePage::TaskbarDateFormat);
  let format = TaskbarDateFormat::WeekdayDayMonthYear;
  select(&mut date_format, Item::DateFormat(format));
  date_format.handle(KeyCode::Enter);
  assert_eq!(date_format.surface_draft().unwrap().date_format, format);

  let mut time_format = draft_app(AppearancePage::TaskbarTimeFormat);
  select(
    &mut time_format,
    Item::TimeFormat(TaskbarTimeFormat::TwelveHour),
  );
  time_format.handle(KeyCode::Enter);
  assert_eq!(
    time_format.surface_draft().unwrap().time_format,
    TaskbarTimeFormat::TwelveHour
  );

  let mut apply_check = draft_app(AppearancePage::TaskbarIcons);
  assert_eq!(apply_check.buttons().len(), 1);
  apply_check.handle(KeyCode::Tab);
  assert!(apply_check.on_buttons);
  apply_check.handle(KeyCode::Enter);
  assert!(apply_check.action.is_some());
}

#[test]
fn apply_keeps_rendering_the_just_applied_draft_until_refresh_replaces_it() {
  // Regression test: pressing Apply used to clear `surface_draft`
  // immediately, which made every toggle/radio on the page flash back to
  // its pre-edit (unchecked/default) state for the whole apply+refresh
  // round trip, since the rows all render from `surface_draft()`.
  let mut application = draft_app(AppearancePage::TaskbarIcons);
  let draft = application.surface_draft.as_mut().unwrap();
  draft.audio_player_enabled = false;
  draft.launcher_enabled = false;

  application.apply_surface_changes();

  let draft = application
    .surface_draft()
    .expect("the just-edited draft must still be present right after Apply");
  assert!(!draft.audio_player_enabled);
  assert!(!draft.launcher_enabled);
  assert!(application.action.is_some());
}
