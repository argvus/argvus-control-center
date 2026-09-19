//! Implements search interaction in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, StatusKind};
use crate::i18n::tr;
use crate::navigation::Page;

/// Renders `draw` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  let line = if app.searching || !app.search.is_empty() {
    Line::from(vec![
      Span::styled(
        format!("{}: ", tr(app.lang, "control_center.search")),
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
      format!(
        "{}: {}",
        tr(app.lang, "control_center.size"),
        app.pending_size
      ),
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
