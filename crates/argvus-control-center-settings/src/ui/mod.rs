pub mod buttons;
pub mod footer;
pub mod header;
pub mod layout;
pub mod list;
pub mod popup;
pub mod search;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::{App, MIN_HEIGHT, MIN_WIDTH};
use crate::i18n::tr;

pub fn draw(app: &mut App, frame: &mut Frame) {
  let area = frame.area();
  app.resize(area.width, area.height);
  Block::new()
    .borders(Borders::ALL)
    .border_style(Style::new().fg(app.theme.border_active))
    .style(
      Style::new()
        .bg(app.theme.background)
        .fg(app.theme.foreground),
    )
    .render(area, frame.buffer_mut());

  if app.too_small() {
    draw_too_small(app, frame, area);
    return;
  }

  let areas = layout::areas(area);
  let buttons = if crate::administration::is_page(app.page()) {
    app.admin_buttons(app.page())
  } else {
    Vec::new()
  };
  if buttons.is_empty() {
    app.set_viewport(areas.body.height as usize);
    header::draw(frame, areas.header, app);
    list::draw(frame, areas.body, app);
  } else {
    let button_height = buttons::height(&buttons, areas.body.width).min(areas.body.height);
    let split =
      Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(areas.body);
    app.set_viewport(split[0].height as usize);
    header::draw(frame, areas.header, app);
    list::draw(frame, split[0], app);
    let selected = app.navigation.current().selected;
    let rows = app.rows().len();
    let focus = selected.checked_sub(rows);
    buttons::draw(
      frame,
      split[1],
      &buttons,
      focus.unwrap_or(usize::MAX),
      &app.theme,
    );
  }
  search::draw(frame, areas.message, app);
  footer::draw(frame, areas.footer, app);
  if let Some(editor) = &app.admin.editor {
    editor.draw(frame, area, app);
  }
  if let Some(message) = app.error_modal.as_deref() {
    popup::draw_error(frame, area, app, message);
  }
  if let Some(action) = app.confirm.as_ref() {
    popup::draw_confirm(frame, area, app, action);
  }
}

fn draw_too_small(app: &App, frame: &mut Frame, area: Rect) {
  let width = area.width.min(52);
  let height = area.height.min(7);
  let popup = layout::centered(area, width, height);
  frame.render_widget(Clear, popup);
  let text = format!(
    "{}\n\n{}: {MIN_WIDTH}x{MIN_HEIGHT}\n{}: {}x{}",
    tr(
      app.lang,
      "A janela do terminal é pequena demais.",
      "Terminal window is too small."
    ),
    tr(app.lang, "Tamanho mínimo", "Minimum size"),
    tr(app.lang, "Tamanho atual", "Current size"),
    area.width,
    area.height
  );
  frame.render_widget(
    Paragraph::new(text).alignment(Alignment::Center).block(
      Block::bordered()
        .border_style(Style::new().fg(app.theme.error))
        .style(
          Style::new()
            .bg(app.theme.background)
            .fg(app.theme.foreground),
        ),
    ),
    popup,
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::navigation::Page;
  use ratatui::{Terminal, backend::TestBackend};
  use serde_json::json;

  fn user_app() -> App {
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    app.navigation.push(Page::User);
    app.admin.accounts = json!({"actor_uid":1000, "shells":["/bin/bash", "/bin/zsh"], "groups":["users", "wheel"],
    "group_details":[
      {"name":"users","gid":1000,"members":[]},
      {"name":"wheel","gid":998,"members":["alice"]}
    ],
    "users":[
      {"user":"root", "uid":0, "gid":0, "name":"root", "shell":"/bin/bash", "primary_group":"root", "groups":[]},
      {"user":"alice", "uid":1000, "gid":1000, "name":"Alice", "shell":"/bin/bash", "primary_group":"users", "groups":["wheel"]}
    ]});
    app.admin.user = app.admin.accounts["users"][1].clone();
    app
  }

  fn button_cell(terminal: &Terminal<TestBackend>, label: &str) -> (u16, u16) {
    let width = terminal.backend().buffer().area.width as usize;
    for (y, row) in terminal
      .backend()
      .buffer()
      .content
      .chunks(width)
      .enumerate()
    {
      let text: String = row.iter().map(|cell| cell.symbol()).collect();
      if let Some(index) = text.find(label)
        && index >= 2
      {
        return (index as u16 - 2, y as u16);
      }
    }
    (0, 0)
  }

  fn button_bg(terminal: &Terminal<TestBackend>, label: &str) -> ratatui::style::Color {
    let (x, y) = button_cell(terminal, label);
    let width = terminal.backend().buffer().area.width;
    terminal.backend().buffer().content[(y * width + x) as usize].bg
  }

  #[test]
  fn button_bar_highlights_only_the_focused_action() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = user_app();
    app.normalize_selection();
    assert_eq!(app.navigation.current().selected, 2);
    let selected_bg = app.theme.selected_background;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    assert_ne!(button_bg(&terminal, "Salvar alterações"), selected_bg);
    assert_ne!(
      button_bg(&terminal, "Excluir usuário (manter home)"),
      selected_bg
    );

    app.navigation.current_mut().selected = 10;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    assert_eq!(button_bg(&terminal, "Salvar alterações"), selected_bg);

    app.navigation.current_mut().selected = 17;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    assert_ne!(button_bg(&terminal, "Salvar alterações"), selected_bg);
    assert_eq!(
      button_bg(&terminal, "Excluir usuário (manter home)"),
      selected_bg
    );
  }

  #[test]
  fn main_page_renders_brand_menu_and_footer() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let rendered = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("ARGVUS"));
    assert!(rendered.contains("Default Apps") || rendered.contains("Apps Padrão"));
    assert!(rendered.contains("Quit") || rendered.contains("Sair"));
  }

  #[test]
  fn minimum_size_message_renders() {
    let mut terminal = Terminal::new(TestBackend::new(42, 10)).unwrap();
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let rendered = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("60x15"));
    assert!(rendered.contains("42x10"));
  }
}
