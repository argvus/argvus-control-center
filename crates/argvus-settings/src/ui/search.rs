use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, StatusKind};
use crate::i18n::tr;
use crate::navigation::Page;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  let line = if app.searching || !app.search.is_empty() {
    Line::from(vec![
      Span::styled(
        format!("{}: ", tr(app.lang, "Buscar", "Search")),
        Style::new().fg(app.theme.accent),
      ),
      Span::styled(
        format!("{}_", app.search),
        Style::new().fg(app.theme.foreground),
      ),
    ])
  } else if let Some(status) = &app.status {
    let color = match status.kind {
      StatusKind::Success => app.theme.success,
      StatusKind::Error => app.theme.error,
    };
    Line::from(Span::styled(&status.text, Style::new().fg(color)))
  } else if let Page::FontSelector(_) = app.page() {
    Line::from(Span::styled(
      format!("{}: {}", tr(app.lang, "Tamanho", "Size"), app.pending_size),
      Style::new().fg(app.theme.muted),
    ))
  } else {
    Line::default()
  };
  frame.render_widget(
    Paragraph::new(line).style(Style::new().bg(app.theme.surface)),
    area,
  );
}
