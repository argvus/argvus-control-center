pub mod footer;
pub mod header;
pub mod layout;
pub mod list;
pub mod popup;
pub mod search;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::{App, MIN_HEIGHT, MIN_WIDTH};
use crate::i18n::tr;

pub fn draw(app: &mut App, frame: &mut Frame) {
  let area = frame.area();
  app.resize(area.width, area.height);
  Block::new()
    .borders(Borders::ALL)
    .border_style(Style::new().fg(app.theme.border_active))
    .style(
      Style::new()
        .bg(app.theme.background)
        .fg(app.theme.foreground),
    )
    .render(area, frame.buffer_mut());

  if app.too_small() {
    draw_too_small(app, frame, area);
    return;
  }

  let areas = layout::areas(area);
  app.set_viewport(areas.body.height as usize);
  header::draw(frame, areas.header, app);
  list::draw(frame, areas.body, app);
  search::draw(frame, areas.message, app);
  footer::draw(frame, areas.footer, app);
  if let Some(message) = app.error_modal.as_deref() {
    popup::draw_error(frame, area, app, message);
  }
}

fn draw_too_small(app: &App, frame: &mut Frame, area: Rect) {
  let width = area.width.min(52);
  let height = area.height.min(7);
  let popup = layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let text = format!(
    "{}\n\n{}: {MIN_WIDTH}x{MIN_HEIGHT}\n{}: {}x{}",
    tr(
      app.lang,
      "A janela do terminal é pequena demais.",
      "Terminal window is too small."
    ),
    tr(app.lang, "Tamanho mínimo", "Minimum size"),
    tr(app.lang, "Tamanho atual", "Current size"),
    area.width,
    area.height
  );
  frame.render_widget(
    Paragraph::new(text).alignment(Alignment::Center).block(
      Block::bordered()
        .border_style(Style::new().fg(app.theme.error))
        .style(
          Style::new()
            .bg(app.theme.background)
            .fg(app.theme.foreground),
        ),
    ),
    popup,
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::navigation::Page;
  use ratatui::{Terminal, backend::TestBackend};

  #[test]
  fn main_page_renders_brand_menu_and_footer() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let rendered = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("ARGVUS"));
    assert!(rendered.contains("Default Apps") || rendered.contains("Apps Padrão"));
    assert!(rendered.contains("Quit") || rendered.contains("Sair"));
  }

  #[test]
  fn minimum_size_message_renders() {
    let mut terminal = Terminal::new(TestBackend::new(42, 10)).unwrap();
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let rendered = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("60x15"));
    assert!(rendered.contains("42x10"));
  }
}
