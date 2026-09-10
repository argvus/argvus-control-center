use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::administration::{Button, ButtonKind};
use crate::theme::Theme;

pub fn height(buttons: &[Button], width: u16) -> u16 {
  if buttons.is_empty() {
    return 0;
  }
  let mut lines = 1usize;
  let mut line_width = 0usize;
  for button in buttons {
    let item = item_width(button);
    if line_width == 0 {
      line_width = item;
    } else if line_width + 1 + item > width as usize {
      lines += 1;
      line_width = item;
    } else {
      line_width += 1 + item;
    }
  }
  lines as u16
}

pub fn draw(frame: &mut Frame, area: Rect, buttons: &[Button], selected: usize, theme: &Theme) {
  if buttons.is_empty() || area.width == 0 || area.height == 0 {
    return;
  }
  let mut lines: Vec<Line<'static>> = Vec::new();
  let mut current: Vec<Span<'static>> = Vec::new();
  let mut line_width = 0usize;
  for (index, button) in buttons.iter().enumerate() {
    let item = item_width(button);
    if line_width > 0 && line_width + 1 + item > area.width as usize {
      lines.push(Line::from(std::mem::take(&mut current)));
      line_width = 0;
    }
    if line_width > 0 {
      current.push(Span::raw(" "));
      line_width += 1;
    }
    current.push(Span::styled(
      format!("[ {} ]", button.label),
      style_for(button, index == selected, theme),
    ));
    line_width += item;
  }
  if !current.is_empty() {
    lines.push(Line::from(current));
  }
  while lines.len() < area.height as usize {
    lines.push(Line::from(""));
  }
  frame.render_widget(
    Paragraph::new(lines).style(Style::new().bg(theme.background)),
    area,
  );
}

fn item_width(button: &Button) -> usize {
  button.label.chars().count() + 4
}

fn style_for(button: &Button, focused: bool, theme: &Theme) -> Style {
  if focused {
    return Style::new()
      .bg(theme.selected_background)
      .fg(theme.selected_foreground)
      .add_modifier(Modifier::BOLD);
  }
  match button.kind {
    ButtonKind::Primary => Style::new().fg(theme.accent),
    ButtonKind::Danger => Style::new().fg(theme.error),
    ButtonKind::Secondary => Style::new().fg(theme.muted),
  }
}
