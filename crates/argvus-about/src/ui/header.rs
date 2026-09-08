use argvus_tui::chrome::{Header, draw_header};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;
use crate::i18n::tr;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
  draw_header(
    frame,
    area,
    &app.theme,
    Header {
      title: tr(app.lang, "Sobre o ARGVUS", "About ARGVUS"),
      version: Some(&app.argvus_version),
      version_label: tr(app.lang, "versão", "version"),
    },
  );
}
