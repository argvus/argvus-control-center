//! Implements popup and editor rendering in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::App;
use crate::app::{PendingAction, pending_action_text};
use crate::i18n::tr;

/// Renders `draw_error` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn draw_error(frame: &mut Frame, area: Rect, app: &App, message: &str) {
  let width = area.width.saturating_sub(8).clamp(30, 64);
  let height = 7.min(area.height);
  let popup = super::layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let title = tr(app.lang, "control_center.error_32d0ef");
  let content = vec![
    Line::from(""),
    Line::from(Span::styled(
      message.to_string(),
      Style::new().fg(app.theme.foreground),
    )),
    Line::from(""),
    Line::from(Span::styled(
      tr(app.lang, "control_center.enter_ok"),
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

/// Renders `draw_confirm` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn draw_confirm(frame: &mut Frame, area: Rect, app: &App, action: &PendingAction) {
  let width = area.width.saturating_sub(8).clamp(34, 72);
  let height = 9.min(area.height);
  let popup = super::layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let (title, body) = pending_action_text(app.lang, action);
  let apply_style = if app.confirm_apply_selected {
    Style::new()
      .fg(app.theme.selected_foreground)
      .bg(app.theme.selected_background)
      .add_modifier(Modifier::BOLD)
  } else {
    Style::new().fg(app.theme.accent)
  };
  let cancel_style = if app.confirm_apply_selected {
    Style::new().fg(app.theme.accent)
  } else {
    Style::new()
      .fg(app.theme.selected_foreground)
      .bg(app.theme.selected_background)
      .add_modifier(Modifier::BOLD)
  };
  let content = vec![
    Line::from(""),
    Line::from(Span::styled(body, Style::new().fg(app.theme.foreground))),
    Line::from(""),
    Line::from(vec![
      Span::styled(tr(app.lang, "control_center.apply"), apply_style),
      Span::raw("  "),
      Span::styled(tr(app.lang, "control_center.cancel_f8378f"), cancel_style),
    ]),
  ];
  frame.render_widget(
    Paragraph::new(content)
      .alignment(Alignment::Center)
      .wrap(ratatui::widgets::Wrap { trim: true })
      .block(
        Block::bordered()
          .title(format!(" {title} "))
          .border_style(Style::new().fg(app.theme.border_active))
          .style(Style::new().bg(app.theme.background)),
      ),
    popup,
  );
}

/// Renders `draw_hostname_input` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn draw_hostname_input(frame: &mut Frame, area: Rect, app: &App) {
  let width = area.width.saturating_sub(8).clamp(40, 52);
  let height = 7.min(area.height);
  let popup = super::layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let title = tr(app.lang, "control_center.hostname");
  let content = vec![
    Line::from(tr(app.lang, "control_center.new_hostname")),
    Line::from(format!("{}_", app.hostname_input)),
    Line::from(tr(app.lang, "control_center.enter_apply_esc_cancel")),
  ];
  frame.render_widget(
    Paragraph::new(content).block(
      Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::new().fg(app.theme.border_active))
        .style(Style::new().bg(app.theme.background)),
    ),
    popup,
  );
}

/// Renders `draw_keybinding_editor` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn draw_keybinding_editor(frame: &mut Frame, area: Rect, app: &App) {
  let popup = super::layout::centered(area, area.width.saturating_sub(10).clamp(46, 70), 13);
  let selected = app.navigation.current().selected;
  let rows = app.rows();
  let name = rows
    .get(selected)
    .map(|row| row.label.as_str())
    .unwrap_or("Keyboard shortcut");
  let (modifiers, key, field) = app.keybinding_editor_state();
  let labels = ["CTRL", "ALT", "SHIFT", "SUPER"];
  let mut content = vec![Line::from(name.to_string()), Line::from("")];
  for (index, label) in labels.into_iter().enumerate() {
    content.push(Line::from(format!(
      "{} [{}] {}",
      if field == index { ">" } else { " " },
      if modifiers[index] { "x" } else { " " },
      label
    )));
  }
  content.extend([
    Line::from(format!(
      "{} Key: [{}]",
      if field == 4 { ">" } else { " " },
      if key.is_empty() { "type a key" } else { key }
    )),
    Line::from(""),
    Line::from(tr(app.lang, "control_center.keybindings_editor_navigation")),
    Line::from(tr(app.lang, "control_center.keybindings_editor_actions")),
  ]);
  let title = tr(app.lang, "control_center.edit_shortcut");
  let title = if title == "control_center.edit_shortcut" {
    "Edit shortcut"
  } else {
    title
  };
  frame.render_widget(
    Paragraph::new(content)
      .block(
        Block::bordered()
          .title(format!(" {title} "))
          .border_style(Style::new().fg(app.theme.border_active))
          .style(Style::new().bg(app.theme.background)),
      )
      .style(Style::new().fg(app.theme.foreground)),
    popup,
  );
}

/// Renders `draw_keybinding_conflict` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn draw_keybinding_conflict(frame: &mut Frame, area: Rect, app: &App) {
  let Some((_, keys, conflicts)) = app.keybinding_conflict_state() else {
    return;
  };
  let popup = super::layout::centered(area, area.width.saturating_sub(10).clamp(46, 70), 10);
  let mut content = vec![
    Line::from(tr(app.lang, "control_center.keybindings_conflict")),
    Line::from(keys.to_string()),
    Line::from(""),
    Line::from(tr(app.lang, "control_center.keybindings_used_by")),
  ];
  for id in conflicts {
    content.push(Line::from(format!("  • {}", app.keybinding_label_for(&id))));
  }
  content.extend([
    Line::from(""),
    Line::from(tr(app.lang, "control_center.keybindings_choose_replace")),
  ]);
  frame.render_widget(
    Paragraph::new(content)
      .block(
        Block::bordered()
          .title(format!(
            " {} ",
            tr(app.lang, "control_center.keybindings_conflict_title")
          ))
          .border_style(Style::new().fg(app.theme.error))
          .style(Style::new().bg(app.theme.background)),
      )
      .style(Style::new().fg(app.theme.foreground)),
    popup,
  );
}
