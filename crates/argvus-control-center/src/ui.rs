use argvus_i18n::tr;
use argvus_tui::chrome::{Header, draw_footer, draw_header, draw_help, draw_too_small};
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
      tr(
        app.lang,
        "A janela do terminal é pequena demais.",
        "Terminal window is too small.",
      ),
      tr(app.lang, "Tamanho mínimo", "Minimum size"),
      tr(app.lang, "Tamanho atual", "Current size"),
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
  }
  if app.help {
    draw_help(
      frame,
      area,
      &app.theme,
      tr(app.lang, "Ajuda", "Help"),
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
    Constraint::Min(1),
    Constraint::Length(1),
  ])
  .split(inner);
  draw_header(
    frame,
    rows[0],
    &app.theme,
    Header {
      title: tr(app.lang, "ARGVUS Control Center", "ARGVUS Control Center"),
      version: None,
      version_label: "",
    },
  );
  let mut selected = 0;
  let lines: Vec<Line<'static>> = app
    .home_rows()
    .into_iter()
    .map(|row| match row {
      HomeRow::Header(label) => {
        let style = Style::new()
          .fg(app.theme.accent)
          .bg(app.theme.background)
          .add_modifier(Modifier::BOLD);
        Line::from(Span::styled(format!("  {label}"), style)).style(style)
      }
      HomeRow::Item { label, .. } => {
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
          format!(" {} {label}", if is_selected { ">" } else { " " }),
          style,
        )])
        .style(style)
      }
    })
    .collect();
  frame.render_widget(Paragraph::new(lines), rows[1].inner(Margin::new(2, 1)));
  draw_footer(
    frame,
    rows[2],
    &app.theme,
    None,
    tr(
      app.lang,
      "↑/↓ Navegar   →/Enter Abrir   s Configuração   ? Ajuda   q Sair",
      "↑/↓ Navigate   →/Enter Open   s Configuration   ? Help   q Quit",
    ),
  );
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
    #[cfg(feature = "about")]
    assert!(text.contains("About"));
    #[cfg(feature = "fonts")]
    assert!(text.contains("Fonts") || text.contains("Fontes"));
    #[cfg(feature = "apps")]
    assert!(text.contains("Default Apps") || text.contains("Apps Padrão"));
    #[cfg(feature = "hardware")]
    assert!(text.contains("Hardware"));
    #[cfg(feature = "services")]
    assert!(text.contains("Services") || text.contains("Serviços"));
    assert!(text.contains("? Help") || text.contains("? Ajuda"));
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
      assert!(text.contains(app.theme.name.as_str()));
      assert!(text.contains("Enter") || text.contains("Abrir"));
      assert!(!text.contains("┌ Network") && !text.contains("┌ Boot"));
    }
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
