use argvus_control_center_core::config::AppConfig;
use argvus_i18n::tr;
use argvus_tui::chrome::{Header, draw_footer, draw_header, draw_help, draw_too_small};
use argvus_tui::text::ellipsize;
use argvus_tui::{MIN_HEIGHT, MIN_WIDTH};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::{App, HomeRow, Route};

pub fn draw(app: &mut App, frame: &mut Frame) {
  let area = frame.area();
  app.resize(area.width, area.height);
  if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
    draw_too_small(
      frame,
      area,
      &app.theme,
      tr(app.lang, "control_center.terminal_window_is_too_small"),
      tr(app.lang, "control_center.minimum_size"),
      tr(app.lang, "control_center.current_size"),
    );
    return;
  }

  match app.route {
    Route::Home => draw_home(app, frame),
    Route::Config => app.config.draw(frame),
    Route::Settings => argvus_control_center_settings::ui::draw(&mut app.settings, frame),
    #[cfg(feature = "about")]
    Route::About => argvus_control_center_about::ui::draw(&mut app.about, frame),
    #[cfg(feature = "hardware")]
    Route::Hardware => app.hardware.draw(frame),
    #[cfg(feature = "services")]
    Route::Services => app.services.draw(frame),
    #[cfg(feature = "network")]
    Route::Network => app.network.draw(frame),
    #[cfg(feature = "audio")]
    Route::Audio => app.audio.draw(frame),
    #[cfg(feature = "bluetooth")]
    Route::Bluetooth => app.bluetooth.draw(frame),
    #[cfg(feature = "boot")]
    Route::Boot => app.boot.draw(frame),
    #[cfg(feature = "packages")]
    Route::Packages => app.packages.draw(frame),
    #[cfg(feature = "storage")]
    Route::Storage => app.storage.draw(frame),
    #[cfg(feature = "diagnostics")]
    Route::Diagnostics => app.diagnostics.draw(frame),
    #[cfg(feature = "power")]
    Route::Power => app.power.draw(frame),
    #[cfg(feature = "session")]
    Route::Session => app.session.draw(frame),
    #[cfg(feature = "displays")]
    Route::Displays => app.displays.draw(frame),
    #[cfg(feature = "appearance")]
    Route::Appearance => app.appearance.draw(frame),
  }
  if app.help {
    draw_help(
      frame,
      area,
      &app.theme,
      tr(app.lang, "control_center.help"),
      &app.help_lines(),
    );
  }
}

fn draw_home(app: &App, frame: &mut Frame) {
  let area = frame.area();
  Block::new()
    .borders(Borders::ALL)
    .border_style(Style::new().fg(app.theme.border_active))
    .bg(app.theme.background)
    .render(area, frame.buffer_mut());
  let inner = area.inner(Margin::new(1, 1));
  let rows = Layout::vertical([
    Constraint::Length(1),
    Constraint::Length(3),
    Constraint::Min(1),
    Constraint::Length(1),
  ])
  .split(inner);
  draw_header(
    frame,
    rows[0],
    &app.theme,
    Header {
      title: tr(app.lang, "control_center.argvus_control_center"),
      version: None,
      version_label: "",
    },
  );
  draw_search(app, frame, rows[1]);
  let mut selected = 0;
  let lines: Vec<Line<'static>> = if app.search_active && !app.search_query.is_empty() {
    let mut lines = vec![Line::from(Span::styled(
      format!("  {}", tr(app.lang, "control_center.search_results")),
      Style::new()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD),
    ))];
    if app.search_result_ids.is_empty() {
      lines.push(Line::from(Span::styled(
        format!("  {}", tr(app.lang, "control_center.search_no_results")),
        Style::new().fg(app.theme.muted),
      )));
    } else {
      for (index, id) in app.search_result_ids.iter().enumerate() {
        if let Some(entry) = app
          .search_registry
          .entries()
          .iter()
          .find(|entry| &entry.id == id)
        {
          let is_selected = index == app.search_selected;
          let label = format!(
            "{} > {}",
            search_category(app.lang, &entry.category),
            search_title(app.lang, entry)
          );
          let style = if is_selected {
            Style::new()
              .fg(app.theme.selected_foreground)
              .bg(app.theme.selected_background)
              .add_modifier(Modifier::BOLD)
          } else {
            Style::new().fg(app.theme.foreground)
          };
          lines.push(
            Line::from(Span::styled(
              format!(
                " {} {}",
                if is_selected { ">" } else { " " },
                ellipsize(&label, rows[2].width.saturating_sub(6) as usize)
              ),
              style,
            ))
            .style(style),
          );
        }
      }
    }
    lines
  } else {
    app
      .home_rows()
      .into_iter()
      .map(|row| match row {
        HomeRow::Header(label) => {
          let style = Style::new()
            .fg(app.theme.accent)
            .bg(app.theme.background)
            .add_modifier(Modifier::BOLD);
          Line::from(Span::styled(
            format!("{}{}", home_icon_for_header(app, label), label),
            style,
          ))
          .style(style)
        }
        HomeRow::Item { label, action } => {
          let is_selected = selected == app.home_selected;
          selected += 1;
          let style = if is_selected {
            Style::new()
              .fg(app.theme.selected_foreground)
              .bg(app.theme.selected_background)
              .add_modifier(Modifier::BOLD)
          } else {
            Style::new()
              .fg(app.theme.foreground)
              .bg(app.theme.background)
          };
          Line::from(vec![Span::styled(
            format!(
              " {} {}{}",
              if is_selected { ">" } else { " " },
              home_icon_for_item(action),
              label
            ),
            style,
          )])
          .style(style)
        }
      })
      .collect()
  };
  frame.render_widget(Paragraph::new(lines), rows[2].inner(Margin::new(2, 1)));
  draw_footer(
    frame,
    rows[3],
    &app.theme,
    None,
    if app.search_active {
      tr(app.lang, "control_center.search_navigation_help")
    } else {
      tr(app.lang, "control_center.home_navigation_search_help")
    },
  );
}

fn draw_search(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
  let value = &app.search_query;
  let style = if app.search_active {
    Style::new()
      .fg(app.theme.foreground)
      .bg(app.theme.background)
  } else {
    Style::new().fg(app.theme.muted).bg(app.theme.background)
  };
  let width = area.width.saturating_sub(4) as usize;
  let prefix = format!(
    "{}{}: ",
    AppConfig::icon(argvus_tui::icons::SEARCH),
    tr(app.lang, "control_center.search_2c43ee")
  );
  let prefix_width = argvus_tui::text::display_width(&prefix);
  let cursor_width = usize::from(app.search_active && app.search_cursor_visible);
  let value_width = width.saturating_sub(prefix_width + cursor_width);
  let value = ellipsize(value, value_width);
  let cursor = if app.search_active && app.search_cursor_visible {
    Span::styled(
      "▌",
      Style::new().fg(app.theme.accent).bg(app.theme.background),
    )
  } else {
    Span::raw("")
  };
  let value_span = Span::styled(value, style);
  frame.render_widget(
    Paragraph::new(Line::from(vec![
      Span::styled(
        prefix,
        Style::new()
          .fg(app.theme.accent)
          .add_modifier(Modifier::BOLD),
      ),
      value_span,
      cursor,
    ]))
    .style(style)
    .block(
      Block::bordered().border_style(Style::new().fg(if app.search_active {
        app.theme.border_active
      } else {
        app.theme.border
      })),
    ),
    area.inner(Margin::new(1, 0)),
  );
}

fn home_icon_for_item(action: usize) -> String {
  let glyph = match action {
    0 => argvus_tui::icons::APPS,
    1 => argvus_tui::icons::FONTS,
    2 => argvus_tui::icons::NETWORK,
    3 => argvus_tui::icons::KEYBOARD,
    4 => argvus_tui::icons::MONITOR,
    5 => argvus_tui::icons::NETWORK,
    6 => argvus_tui::icons::AUDIO,
    7 => argvus_tui::icons::LINK,
    8 => argvus_tui::icons::BOOT,
    9 => argvus_tui::icons::PACKAGES,
    10 => argvus_tui::icons::SETTINGS,
    11 => argvus_tui::icons::SETTINGS,
    12 => argvus_tui::icons::STORAGE,
    13 => argvus_tui::icons::DIAGNOSTICS,
    14 => argvus_tui::icons::INFO,
    15 => argvus_tui::icons::SETTINGS,
    16 => argvus_tui::icons::POWER,
    17 => argvus_tui::icons::REFRESH,
    18 => argvus_tui::icons::MONITOR,
    19 => argvus_tui::icons::PALETTE,
    20 => argvus_tui::icons::MOUSE,
    21 => argvus_tui::icons::KEYBOARD,
    _ => "",
  };
  let icon = AppConfig::icon(glyph);
  argvus_tui::icons::icon_label(icon, "")
}

fn home_icon_for_header(app: &App, label: &str) -> String {
  let glyph = [
    ("control_center.language_region", argvus_tui::icons::NETWORK),
    ("control_center.appearance", argvus_tui::icons::PALETTE),
    ("control_center.applications", argvus_tui::icons::APPS),
    ("control_center.hardware", argvus_tui::icons::MONITOR),
    ("control_center.power_session", argvus_tui::icons::POWER),
    ("control_center.connectivity", argvus_tui::icons::NETWORK),
    ("control_center.audio", argvus_tui::icons::AUDIO),
    ("control_center.system", argvus_tui::icons::SETTINGS),
    ("control_center.preferences", argvus_tui::icons::SETTINGS),
  ]
  .into_iter()
  .find_map(|(key, glyph)| (tr(app.lang, key) == label).then_some(glyph))
  .unwrap_or("▸");
  argvus_tui::icons::icon_label(AppConfig::icon(glyph), "")
}

fn search_category(lang: argvus_i18n::Lang, category: &str) -> String {
  let key = format!("control_center.{category}");
  let translated = tr(lang, &key);
  if translated == key {
    category.to_string()
  } else {
    translated.to_string()
  }
}

fn search_title(
  lang: argvus_i18n::Lang,
  entry: &argvus_control_center_core::search::SearchEntry,
) -> String {
  let segment = entry
    .route
    .rsplit('/')
    .next()
    .unwrap_or(entry.title.as_str());
  let key = match segment {
    "fonts" => "control_center.fonts",
    "default-apps" => "control_center.default_apps",
    "wifi" => "control_center.wi_fi",
    "plymouth" => "control_center.plymouth",
    "bootloader" => "control_center.bootloader_f1a3c5",
    "initramfs" => "control_center.initramfs",
    "summary" => "control_center.summary",
    "status" => "control_center.status",
    "interfaces" => "control_center.interfaces",
    "ethernet" => "control_center.ethernet",
    "vpn" => "control_center.vpn",
    "dns" => "control_center.dns",
    "proxy" => "control_center.proxy",
    "output" => "control_center.output",
    "input" => "control_center.input",
    "devices" => "control_center.devices",
    "state" => "control_center.state",
    "pair" => "control_center.pair",
    "cpu" => "control_center.hardware_cpu",
    "gpu" => "control_center.hardware_gpu",
    "memory" => "control_center.memory",
    "power" => "control_center.power",
    "system" => "control_center.system",
    "user" => "control_center.user",
    "failed" => "control_center.failed",
    "logs" => "control_center.logs",
    "search" => "control_center.search_2c43ee",
    "installed" => "control_center.installed_e91b6d",
    "updates" => "control_center.updates",
    "orphans" => "control_center.orphans_29aae8",
    "cache" => "control_center.cache",
    "aur" => "control_center.aur",
    "history" => "control_center.history",
    "downgrade" => "control_center.downgrade",
    "mirrors" => "control_center.mirrors",
    "disks" => "control_center.disks",
    "partitions" => "control_center.partitions",
    "filesystems" => "control_center.filesystems",
    "mounts" => "control_center.mount_points",
    "smart" => "control_center.smart",
    "usage" => "control_center.disk_usage",
    "keybindings" => "control_center.keyboard_shortcuts",
    _ => return entry.title.clone(),
  };
  let translated = tr(lang, key);
  if translated == key {
    entry.title.clone()
  } else {
    translated.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::InitialRoute;
  use ratatui::{Terminal, backend::TestBackend};

  #[test]
  fn home_renders_existing_and_phase_two_destinations() {
    let mut app = App::new(InitialRoute::Home);
    let mut terminal = Terminal::new(TestBackend::new(90, 32)).unwrap();
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("ARGVUS"));
    assert!(text.contains("Control Center"));
    assert!(text.contains("/ Search") || text.contains("/ Pesquisar") || text.contains("Search"));
  }

  #[test]
  fn domain_pages_use_the_shared_chrome() {
    let cases: [(Route, [&str; 2]); _] = [
      #[cfg(feature = "network")]
      (Route::Network, ["Network", "Rede"]),
      #[cfg(feature = "boot")]
      (Route::Boot, ["Boot", "Boot"]),
      #[cfg(feature = "packages")]
      (Route::Packages, ["Packages", "Pacotes"]),
    ];
    for (route, marker) in cases {
      let mut app = App::new(InitialRoute::Home);
      app.route = route;
      let mut terminal = Terminal::new(TestBackend::new(90, 25)).unwrap();
      terminal.draw(|frame| draw(&mut app, frame)).unwrap();
      let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
      assert!(text.contains("ARGVUS"));
      assert!(text.contains(marker[0]) || text.contains(marker[1]));
      assert!(text.contains("Enter") || text.contains("Abrir"));
      assert!(!text.contains("┌ Network") && !text.contains("┌ Boot"));
    }
  }

  #[test]
  fn home_renders_the_live_global_query_and_result() {
    let mut app = App::new(InitialRoute::Home);
    app.begin_global_search();
    app.update_global_search("wifi".into());
    let mut terminal = Terminal::new(TestBackend::new(90, 32)).unwrap();
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("wifi"));
    assert!(text.contains("Wi-Fi"));
    assert!(text.contains("▌"));
  }

  #[test]
  fn config_screen_renders_with_chrome_and_checkbox() {
    let mut app = App::new(InitialRoute::Home);
    app.route = Route::Config;
    let mut terminal = Terminal::new(TestBackend::new(90, 24)).unwrap();
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("ARGVUS"));
    assert!(text.contains("Configuração") || text.contains("Configuration"));
    assert!(text.contains("[✓]") || text.contains("[ ]"));
    assert!(text.contains("Icones") || text.contains("Icons"));
  }
}
