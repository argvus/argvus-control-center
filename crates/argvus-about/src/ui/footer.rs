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
      "q Sair | ←/→ Abas | ↑/↓ Rolar | Enter Abrir | ? Ajuda",
      "q Quit | ←/→ Tabs | ↑/↓ Scroll | Enter Open | ? Help",
    )
  } else {
    tr(
      app.lang,
      "q Sair  |  ←/→ Abas  |  ↑/↓ Navegar  |  Enter Abrir  |  PgUp/PgDn Rolar  |  ? Ajuda",
      "q Quit  |  ←/→ Tabs  |  ↑/↓ Navigate  |  Enter Open  |  PgUp/PgDn Scroll  |  ? Help",
    )
  };
  draw_footer(frame, area, &app.theme, status, hints);
}
