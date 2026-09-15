use argvus_tui::chrome::{MessageKind, draw_footer};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{App, StatusKind};
use crate::i18n::tr;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  let status = app.status.as_ref().map(|status| {
    let kind = match status.kind {
      StatusKind::Info => MessageKind::Info,
      StatusKind::Success => MessageKind::Success,
      StatusKind::Error => MessageKind::Error,
    };
    (status.text.as_str(), kind)
  });
  let hints = if area.width < 110 {
    tr(
      app.lang,
      "control_center.q_quit_esc_back_tabs_scroll_enter_open_help",
    )
  } else {
    tr(
      app.lang,
      "control_center.q_quit_esc_back_tabs_navigate_enter_open_pgup_pgdn_scroll_help",
    )
  };
  draw_footer(frame, area, &app.theme, status, hints);
}
