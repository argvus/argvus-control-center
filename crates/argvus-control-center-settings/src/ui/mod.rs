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
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

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
  let buttons = app.page_buttons();
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
  if app.hostname_editing {
    popup::draw_hostname_input(frame, area, app);
  }
  if let Some(editor) = &app.admin.editor {
    editor.draw(frame, area, app);
  }
  if let Some(message) = app.error_modal.as_deref() {
    popup::draw_error(frame, area, app, message);
  }
  if let Some(action) = app.confirm.as_ref() {
    popup::draw_confirm(frame, area, app, action);
  }
  draw_task_window(app, frame, area);
}

fn draw_task_window(app: &App, frame: &mut Frame, area: Rect) {
  if !app.task_open {
    return;
  }
  let Some(output) = app.task_live.as_ref().map(|live| live.output()) else {
    return;
  };
  let popup = layout::centered(
    area,
    crate::app::TASK_POPUP_WIDTH,
    crate::app::TASK_POPUP_HEIGHT,
  );
  let total = crate::app::task_wrapped_lines(&output);
  let shown_total = total.max(1);
  let current = (app.task_scroll as usize + 1).min(shown_total);
  frame.render_widget(Clear, popup);
  frame.render_widget(
    Paragraph::new(output.as_str())
      .block(
        Block::new()
          .borders(Borders::ALL)
          .title(Line::styled(
            format!(
              " {} · {} {} {} {} ",
              tr(app.lang, "Processo", "Process",),
              tr(app.lang, "linha", "line"),
              current,
              tr(app.lang, "de", "of"),
              shown_total,
            ),
            Style::new().fg(app.theme.accent),
          ))
          .border_style(Style::new().fg(app.theme.border_active))
          .style(
            Style::new()
              .bg(app.theme.background)
              .fg(app.theme.foreground),
          )
          .title_bottom(Line::from(tr(
            app.lang,
            "↑↓ rolar · PgUp/PgDn · Home/End · Esc fechar",
            "↑↓ scroll · PgUp/PgDn · Home/End · Esc close",
          ))),
      )
      .scroll((app.task_scroll, 0))
      .wrap(Wrap { trim: false }),
    popup,
  );
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
  use crossterm::event::{Event, KeyCode, KeyEvent};
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
  fn reset_pages_render_a_reset_defaults_button() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    for page in [Page::DefaultApps, Page::Fonts] {
      let mut app = App::new(page);
      app.error_modal = None;
      terminal.draw(|frame| draw(&mut app, frame)).unwrap();
      let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
      assert!(
        rendered.contains("[ Restaurar padrões ]") || rendered.contains("[ Reset Defaults ]")
      );
    }
  }

  #[test]
  fn hostname_editing_renders_popup_with_typed_value() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = App::new(Page::Hostname);
    app.error_modal = None;
    app.hostname_editing = true;
    app.hostname_input = "mybox".into();
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let rendered = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("mybox_"), "typed value must render");
    assert!(
      rendered.contains("Enter apply") || rendered.contains("Enter aplicar"),
      "popup hint must render"
    );
    assert!(
      rendered.contains("Current hostname") || rendered.contains("Hostname atual"),
      "list row must still show the current hostname"
    );
  }

  #[test]
  fn tab_enters_reset_button_and_enter_opens_confirm() {
    let mut app = App::new(Page::DefaultApps);
    app.error_modal = None;
    assert!(!app.on_buttons());
    app.normalize_selection();

    crate::event::handle(&mut app, Event::Key(KeyEvent::from(KeyCode::Tab)));
    assert!(app.on_buttons());

    crate::event::handle(&mut app, Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(matches!(
      app.confirm,
      Some(crate::app::PendingAction::ResetApps)
    ));

    crate::event::handle(&mut app, Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(app.confirm.is_none());

    crate::event::handle(&mut app, Event::Key(KeyEvent::from(KeyCode::Tab)));
    assert!(!app.on_buttons());
    assert_eq!(app.navigation.current().selected, 0);
  }

  #[test]
  fn font_selector_page_has_reset_button_and_search_still_works() {
    let mut app = App::new(Page::Fonts);
    app.error_modal = None;
    app
      .navigation
      .push(Page::FontSelector(crate::config::fonts::FontTarget::Apps));
    assert!(matches!(
      app.page_buttons().first(),
      Some(button) if button.label.ends_with("Defaults") || button.label.ends_with("padrões")
    ));
    crate::event::handle(&mut app, Event::Key(KeyEvent::from(KeyCode::Tab)));
    assert!(app.on_buttons());
    crate::event::handle(&mut app, Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(app.confirm.is_some());
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

  #[test]
  fn task_window_renders_while_open() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    let live = argvus_control_center_core::process::LiveProcess::new();
    live.push_line("locale-gen");
    app.task_live = Some(live);
    app.task_open = true;
    terminal.draw(|frame| draw(&mut app, frame)).unwrap();
    let rendered = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(rendered.contains("locale-gen"), "live line must render");
    assert!(
      rendered.contains("Process") || rendered.contains("Processo"),
      "process window title must render"
    );
  }
}
