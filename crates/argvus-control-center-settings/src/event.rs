use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::App;

pub fn handle(app: &mut App, event: Event) {
  match event {
    Event::Resize(width, height) => app.resize(width, height),
    Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key),
    _ => {}
  }
}

fn handle_key(app: &mut App, key: KeyEvent) {
  if app.confirm.is_some() {
    match key.code {
      KeyCode::Enter => app.confirm_accept(),
      KeyCode::Esc => app.cancel_modal(),
      KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
        app.toggle_confirm_button()
      }
      _ => {}
    }
    return;
  }

  if app.error_modal.is_some() {
    if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
      app.error_modal = None;
    }
    return;
  }

  if app.hostname_editing {
    app.hostname_input(key.code);
    return;
  }

  if app.admin.editor.is_some() {
    app.admin_input(key);
    return;
  }

  if app.searching {
    match key.code {
      KeyCode::Esc => app.back(),
      KeyCode::Enter => {
        app.searching = false;
        app.open_or_apply();
      }
      KeyCode::Backspace => app.pop_search(),
      KeyCode::Down => app.move_selection(1),
      KeyCode::Up => app.move_selection(-1),
      KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
        app.push_search(character)
      }
      _ => {}
    }
    return;
  }

  match key.code {
    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
    KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => app.open_or_apply(),
    KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => app.back(),
    KeyCode::Char('/') => app.begin_search(),
    KeyCode::Char('r') => app.reset_current(),
    KeyCode::Char(' ') => app.toggle_current(),
    KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_size(1),
    KeyCode::Char('-') => app.adjust_size(-1),
    KeyCode::Home => {
      let selected = app.navigation.current().selected;
      app.move_selection(-(selected as isize));
    }
    KeyCode::End => {
      let count = app.rows().len();
      let selected = app.navigation.current().selected;
      app.move_selection(count.saturating_sub(selected + 1) as isize);
    }
    KeyCode::PageUp => app.move_selection(-(app.viewport as isize)),
    KeyCode::PageDown => app.move_selection(app.viewport as isize),
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resize_is_forwarded() {
    let mut app = App::new(crate::navigation::Page::Main);
    handle(&mut app, Event::Resize(100, 30));
    assert_eq!((app.width, app.height), (100, 30));
  }
}
