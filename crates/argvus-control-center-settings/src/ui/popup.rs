use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::App;
use crate::i18n::tr;

pub fn draw_error(frame: &mut Frame, area: Rect, app: &App, message: &str) {
  let width = area.width.saturating_sub(8).clamp(30, 64);
  let height = 7.min(area.height);
  let popup = super::layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let title = tr(app.lang, " Erro ", " Error ");
  let content = vec![
    Line::from(""),
    Line::from(Span::styled(
      message.to_string(),
      Style::new().fg(app.theme.foreground),
    )),
    Line::from(""),
    Line::from(Span::styled(
      tr(app.lang, "Enter  OK", "Enter  OK"),
      Style::new()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD),
    )),
  ];
  frame.render_widget(
    Paragraph::new(content)
      .alignment(Alignment::Center)
      .wrap(ratatui::widgets::Wrap { trim: true })
      .block(
        Block::bordered()
          .title(title)
          .border_style(Style::new().fg(app.theme.error))
          .style(Style::new().bg(app.theme.background)),
      ),
    popup,
  );
}
