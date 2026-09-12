use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use argvus_tui::text::{display_width, truncate_to_width};

use crate::app::App;
use crate::i18n::tr;
use crate::navigation::Page;

const RIGHT_GUTTER: usize = 1;

struct RowLayout {
  marker: String,
  label: String,
  detail: String,
  padding: usize,
  right_gutter: usize,
}

fn row_render_area(area: Rect) -> Rect {
  // Keep the final terminal cell completely outside the Paragraph. A VS16
  // width disagreement in affected Ratatui/crossterm combinations can drift
  // a row by one physical cell; an unrendered guard cell prevents that drift
  // from ever reaching the outer border. This is deliberately different from
  // RIGHT_GUTTER, which is styled as part of the selected row.
  Rect {
    width: area.width.saturating_sub(1),
    ..area
  }
}

fn layout_row(
  width: usize,
  marker: &str,
  label: &str,
  detail: &str,
  truncate_detail: bool,
) -> RowLayout {
  // The body area begins immediately after the outer border cell. Keep one
  // terminal column free on the right so right-aligned values do not touch
  // the border. The gutter is part of the row itself so selected rows keep
  // their background color all the way across.
  let right_gutter = RIGHT_GUTTER.min(width);
  let content_width = width.saturating_sub(right_gutter);

  let mut detail = detail.to_string();
  if truncate_detail && display_width(&detail) > content_width / 2 {
    detail = truncate_to_width(&detail, (content_width / 2).saturating_sub(1)) + "…";
  }
  let reserved = display_width(&detail).saturating_add(5);
  let label_width = content_width.saturating_sub(reserved).max(8);
  let mut label = label.to_string();
  if display_width(&label) > label_width {
    label = truncate_to_width(&label, label_width.saturating_sub(1)) + "…";
  }
  let left_len = display_width(marker) + display_width(&label) + 2;
  let padding = content_width.saturating_sub(left_len + display_width(&detail));
  RowLayout {
    marker: marker.to_string(),
    label,
    detail,
    padding,
    right_gutter,
  }
}

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  let rows = app.rows();
  let location = app.navigation.current();
  let render_area = row_render_area(area);
  let width = render_area.width as usize;
  let lines: Vec<Line<'static>> = rows
    .into_iter()
    .skip(location.scroll)
    .take(area.height as usize)
    .enumerate()
    .map(|(visible, row)| {
      let index = location.scroll + visible;
      let selectable = app.row_selectable(index);
      let selected = index == location.selected && selectable;
      let marker = if row.current {
        "●"
      } else if selected {
        ">"
      } else {
        " "
      };
      let layout = layout_row(
        width,
        marker,
        &row.label,
        row.detail.as_deref().unwrap_or_default(),
        crate::administration::is_page(app.page()),
      );
      let RowLayout {
        marker,
        label,
        detail,
        padding,
        right_gutter,
      } = layout;
      let style = if selected {
        Style::new()
          .bg(app.theme.selected_background)
          .fg(app.theme.selected_foreground)
          .add_modifier(Modifier::BOLD)
      } else if !selectable {
        Style::new().bg(app.theme.background).fg(app.theme.muted)
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
        Span::styled(" ".repeat(right_gutter), style),
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
  frame.render_widget(paragraph, render_area);
}

#[cfg(test)]
mod tests {
  use super::*;
  use ratatui::{Terminal, backend::TestBackend};

  fn locale_region_rows() -> Vec<(String, String)> {
    vec![
      ("🌍 Time Zone".to_string(), "America/Sao_Paulo".to_string()),
      ("🕒 Date & Time".to_string(), "00:54:59".to_string()),
      ("🌐 Regional Locale".to_string(), "pt_BR.UTF-8".to_string()),
      ("🗂️ System Locales".to_string(), "2 / 500".to_string()),
      ("⌨️ Keyboard".to_string(), "br  ·  us-intl".to_string()),
    ]
  }

  fn render_lines(rows: &[(String, String)], width: u16) -> String {
    let lines: Vec<Line<'static>> = rows
      .iter()
      .enumerate()
      .map(|(index, (label, detail))| {
        let marker = if index == 1 { ">" } else { " " };
        let layout = layout_row(width as usize, marker, label, detail, false);
        let mut text = format!(
          " {marker} {label}",
          marker = layout.marker,
          label = layout.label
        );
        text.push_str(&" ".repeat(layout.padding));
        text.push_str(&layout.detail);
        text.push_str(&" ".repeat(layout.right_gutter));
        Line::from(text)
      })
      .collect();
    let mut terminal = Terminal::new(TestBackend::new(width + 1, rows.len() as u16)).unwrap();
    terminal
      .draw(|frame| {
        frame.render_widget(
          Paragraph::new(lines),
          Rect::new(0, 0, width, rows.len() as u16),
        );
      })
      .unwrap();
    terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect()
  }

  #[test]
  fn wide_emoji_icons_no_longer_push_details_off_screen() {
    let rows = locale_region_rows();
    let rendered = render_lines(&rows, 100);
    assert!(
      rendered.contains("America/Sao_Paulo"),
      "tz must render whole"
    );
    assert!(rendered.contains("00:54:59"), "clock must render whole");
    assert!(rendered.contains("pt_BR.UTF-8"), "locale must render whole");
    assert!(rendered.contains("2 / 500"), "count must render whole");
    assert!(
      rendered.contains("br  ·  us-intl"),
      "keyboard must render whole"
    );
  }

  #[test]
  fn layout_row_keeps_wide_size_fit_in_columns() {
    let rows = locale_region_rows();
    for (label, detail) in &rows {
      let layout = layout_row(100, " ", label, detail, false);
      let total = display_width(&layout.marker)
        + display_width(&layout.label)
        + 2
        + layout.padding
        + display_width(&layout.detail)
        + layout.right_gutter;
      assert!(total <= 100, "{label}: total {total} overflows");
      assert_eq!(layout.detail, *detail, "{label}: detail must stay intact");
    }
  }

  #[test]
  fn layout_row_reserves_one_column_after_detail() {
    let width = 100;
    let layout = layout_row(width, ">", "Fonte da Taskbar", "IBM Plex Mono · 13", false);
    let content_width = display_width(&layout.marker)
      + display_width(&layout.label)
      + 2
      + layout.padding
      + display_width(&layout.detail);

    assert_eq!(layout.right_gutter, RIGHT_GUTTER);
    assert_eq!(content_width + layout.right_gutter, width);
    assert_eq!(content_width, width - RIGHT_GUTTER);
  }

  #[test]
  fn rendered_row_keeps_last_cell_blank() {
    let width = 40u16;
    let layout = layout_row(
      width as usize,
      ">",
      "Fonte da Taskbar",
      "IBM Plex Mono · 13",
      false,
    );
    let style = Style::new().bg(ratatui::style::Color::Blue);
    let line = Line::from(vec![
      Span::styled(
        format!(
          " {marker} {label}",
          marker = layout.marker,
          label = layout.label
        ),
        style,
      ),
      Span::styled(" ".repeat(layout.padding), style),
      Span::styled(layout.detail, style),
      Span::styled(" ".repeat(layout.right_gutter), style),
    ]);
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal
      .draw(|frame| frame.render_widget(Paragraph::new(line), Rect::new(0, 0, width, 1)))
      .unwrap();

    let buffer = terminal.backend().buffer();
    let last = &buffer.content[(width - 1) as usize];
    assert_eq!(last.symbol(), " ");
    assert_eq!(last.bg, ratatui::style::Color::Blue);

    let rendered = buffer
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("IBM Plex Mono · 13 "));
  }

  #[test]
  fn row_render_area_keeps_a_hard_right_guard_cell() {
    let outer = Rect::new(4, 2, 40, 3);
    let render = row_render_area(outer);
    assert_eq!(render.x, outer.x);
    assert_eq!(render.y, outer.y);
    assert_eq!(render.height, outer.height);
    assert_eq!(render.width, 39);
    assert_eq!(render.right(), outer.right() - 1);
  }

  #[test]
  fn hard_guard_cell_is_not_painted_by_selected_row() {
    let width = 40u16;
    let area = Rect::new(0, 0, width, 1);
    let render_area = row_render_area(area);
    let selected = Style::new().bg(ratatui::style::Color::Blue);
    let line = Line::from(Span::styled(" selected row", selected)).style(selected);
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal
      .draw(|frame| frame.render_widget(Paragraph::new(line), render_area))
      .unwrap();

    let buffer = terminal.backend().buffer();
    let guard = &buffer.content[(width - 1) as usize];
    assert_eq!(guard.symbol(), " ");
    assert_ne!(guard.bg, ratatui::style::Color::Blue);
  }
}
