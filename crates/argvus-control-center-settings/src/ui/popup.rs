use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::App;
use crate::app::{PendingAction, pending_action_text};
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
      Span::styled(tr(app.lang, "[ Aplicar ]", "[ Apply ]"), apply_style),
      Span::raw("  "),
      Span::styled(tr(app.lang, "[ Cancelar ]", "[ Cancel ]"), cancel_style),
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

pub fn draw_hostname_input(frame: &mut Frame, area: Rect, app: &App) {
  let width = area.width.saturating_sub(8).clamp(40, 52);
  let height = 7.min(area.height);
  let popup = super::layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let title = tr(app.lang, "Hostname", "Hostname");
  let content = vec![
    Line::from(tr(app.lang, "Novo hostname:", "New hostname:")),
    Line::from(format!("{}_", app.hostname_input)),
    Line::from(tr(
      app.lang,
      "Enter aplicar   Esc cancelar",
      "Enter apply   Esc cancel",
    )),
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
