use argvus_theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

pub struct Header<'a> {
  pub title: &'a str,
  pub version: Option<&'a str>,
  pub version_label: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub enum MessageKind {
  Info,
  Success,
  Error,
}

pub fn draw_header(frame: &mut Frame, area: Rect, theme: &Theme, header: Header<'_>) {
  Block::new()
    .bg(theme.surface)
    .render(area, frame.buffer_mut());
  let inner = area.inner(Margin::new(1, 0));
  let version_width = header
    .version
    .map(|version| header.version_label.chars().count() + version.chars().count() + 3)
    .unwrap_or(0);
  let requested = version_width + theme.name.chars().count();
  let left_minimum = 10 + header.title.chars().count();
  let right_width = requested.min((inner.width as usize).saturating_sub(left_minimum));
  let columns =
    Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_width as u16)]).split(inner);
  frame.render_widget(
    Paragraph::new(Line::from(vec![
      Span::styled(
        "ARGVUS",
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
      ),
      Span::raw("  "),
      Span::styled(header.title.to_string(), Style::new().fg(theme.foreground)),
    ])),
    columns[0],
  );
  let mut right = Vec::new();
  if let Some(version) = header.version {
    right.push(Span::styled(
      format!("{} ", header.version_label),
      Style::new().fg(theme.muted),
    ));
    right.push(Span::styled(
      version.to_string(),
      Style::new().fg(theme.foreground),
    ));
    right.push(Span::raw("  "));
  }
  right.push(Span::styled(
    theme.name.clone(),
    Style::new().fg(theme.warning),
  ));
  frame.render_widget(
    Paragraph::new(Line::from(right)).alignment(Alignment::Right),
    columns[1],
  );
}

pub fn draw_footer(
  frame: &mut Frame,
  area: Rect,
  theme: &Theme,
  status: Option<(&str, MessageKind)>,
  hints: &str,
) {
  Block::new()
    .bg(theme.surface)
    .render(area, frame.buffer_mut());
  let rows = if area.height > 1 {
    Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area)
  } else {
    Layout::vertical([Constraint::Length(0), Constraint::Length(1)]).split(area)
  };
  if let Some((text, kind)) = status {
    let color = match kind {
      MessageKind::Info => theme.muted,
      MessageKind::Success => theme.success,
      MessageKind::Error => theme.error,
    };
    frame.render_widget(
      Paragraph::new(text).style(Style::new().fg(color)),
      rows[0].inner(Margin::new(1, 0)),
    );
  }
  frame.render_widget(
    Paragraph::new(hints)
      .alignment(Alignment::Right)
      .style(Style::new().fg(theme.muted).bg(theme.surface)),
    rows[1].inner(Margin::new(1, 0)),
  );
}

pub fn draw_help(frame: &mut Frame, area: Rect, theme: &Theme, title: &str, lines: &[String]) {
  let width = area.width.saturating_sub(8).clamp(36, 68);
  let height = (lines.len() as u16 + 4).min(area.height).max(5);
  let popup = centered(area, width, height);
  frame.render_widget(Clear, popup);
  frame.render_widget(
    Paragraph::new(lines.join("\n"))
      .wrap(Wrap { trim: false })
      .style(Style::new().fg(theme.foreground).bg(theme.background))
      .block(
        Block::new()
          .borders(Borders::ALL)
          .title(format!(" {title} "))
          .border_style(Style::new().fg(theme.border_active)),
      ),
    popup,
  );
}

pub fn draw_too_small(
  frame: &mut Frame,
  area: Rect,
  theme: &Theme,
  message: &str,
  minimum: &str,
  current: &str,
) {
  Block::new()
    .bg(theme.background)
    .render(area, frame.buffer_mut());
  let popup = centered(area, area.width.min(54), area.height.min(7));
  frame.render_widget(Clear, popup);
  frame.render_widget(
    Paragraph::new(format!(
      "{message}\n\n{minimum}: {}x{}\n{current}: {}x{}",
      crate::MIN_WIDTH,
      crate::MIN_HEIGHT,
      area.width,
      area.height
    ))
    .alignment(Alignment::Center)
    .block(
      Block::new()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.error))
        .style(Style::new().bg(theme.background).fg(theme.foreground)),
    ),
    popup,
  );
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
  Rect::new(
    area.x + area.width.saturating_sub(width) / 2,
    area.y + area.height.saturating_sub(height) / 2,
    width.min(area.width),
    height.min(area.height),
  )
}
