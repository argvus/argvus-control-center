use argvus_tui::chrome::{Header, draw_header};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;
use crate::i18n::tr;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  let title = format!(
    "{} > {}",
    tr(app.lang, "control_center.argvus_control_center"),
    app.active_tab.label(app.lang),
  );
  draw_header(
    frame,
    area,
    &app.theme,
    Header {
      title: &title,
      version: Some(&app.argvus_version),
      version_label: tr(app.lang, "control_center.version"),
    },
  );
}
