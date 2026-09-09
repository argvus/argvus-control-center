use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::i18n::tr;
use crate::navigation::Page;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  let rows = app.rows();
  let location = app.navigation.current();
  let width = area.width as usize;
  let lines: Vec<Line<'static>> = rows
    .into_iter()
    .skip(location.scroll)
    .take(area.height as usize)
    .enumerate()
    .map(|(visible, row)| {
      let index = location.scroll + visible;
      let selected = index == location.selected;
      let marker = if row.current {
        "●"
      } else if selected {
        ">"
      } else {
        " "
      };
      let mut detail = row.detail.unwrap_or_default();
      if crate::administration::is_page(app.page()) && detail.chars().count() > width / 2 {
        detail = detail
          .chars()
          .take((width / 2).saturating_sub(1))
          .collect::<String>()
          + "…";
      }
      let reserved = detail.chars().count().saturating_add(5);
      let label_width = width.saturating_sub(reserved).max(8);
      let mut label = row.label;
      if label.chars().count() > label_width {
        label = label
          .chars()
          .take(label_width.saturating_sub(1))
          .collect::<String>()
          + "…";
      }
      let padding = width.saturating_sub(
        marker.chars().count() + label.chars().count() + detail.chars().count() + 3,
      );
      let style = if selected {
        Style::new()
          .bg(app.theme.selected_background)
          .fg(app.theme.selected_foreground)
          .add_modifier(Modifier::BOLD)
      } else {
        Style::new().bg(app.theme.background).fg(if row.current {
          app.theme.accent
        } else {
          app.theme.foreground
        })
      };
      Line::from(vec![
        Span::styled(format!(" {marker} {label}"), style),
        Span::styled(" ".repeat(padding), style),
        Span::styled(detail, style),
      ])
      .style(style)
    })
    .collect();

  let paragraph = if lines.is_empty() {
    let message = if matches!(app.page(), Page::AppSelector(_)) && app.search.is_empty() {
      tr(
        app.lang,
        "Nenhum aplicativo instalado detectado",
        "No installed applications detected",
      )
    } else {
      tr(app.lang, "Nenhum item encontrado", "No matching items")
    };
    Paragraph::new(message).style(Style::new().fg(app.theme.muted).bg(app.theme.background))
  } else {
    Paragraph::new(lines).style(Style::new().bg(app.theme.background))
  };
  frame.render_widget(paragraph, area);
}
