use argvus_i18n::tr;
use argvus_tui::chrome::{Header, draw_footer, draw_header, draw_help, draw_too_small};
use argvus_tui::{MIN_HEIGHT, MIN_WIDTH};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::{App, Route};

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
    Route::Settings => argvus_control_center_settings::ui::draw(&mut app.settings, frame),
    Route::About => argvus_control_center_about::ui::draw(&mut app.about, frame),
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
      title: tr(app.lang, "Central de Controle", "Control Center"),
      version: None,
      version_label: "",
    },
  );
  let lines: Vec<Line<'static>> = app
    .home_rows()
    .into_iter()
    .enumerate()
    .map(|(index, label)| {
      let selected = index == app.home_selected;
      let style = if selected {
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
        format!(" {} {label}", if selected { ">" } else { " " }),
        style,
      )])
      .style(style)
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
      "↑/↓ Navegar   →/Enter Abrir   ? Ajuda   q Sair",
      "↑/↓ Navigate   →/Enter Open   ? Help   q Quit",
    ),
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::InitialRoute;
  use ratatui::{Terminal, backend::TestBackend};

  #[test]
  fn home_renders_all_three_destinations() {
    let mut app = App::new(InitialRoute::Home);
    let mut terminal = Terminal::new(TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("About"));
    assert!(text.contains("Fonts") || text.contains("Fontes"));
    assert!(text.contains("Default Apps") || text.contains("Apps Padrão"));
  }
}
