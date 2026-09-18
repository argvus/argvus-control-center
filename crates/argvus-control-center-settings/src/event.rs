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
  if app.task_open {
    match key.code {
      KeyCode::Esc => {
        app.task_open = false;
        app.task_live = None;
        app.task_scroll = 0;
        app.task_follow = false;
      }
      KeyCode::Up | KeyCode::Char('k') => {
        app.task_follow = false;
        app.task_scroll = app.task_scroll.saturating_sub(1);
      }
      KeyCode::Down | KeyCode::Char('j') => {
        app.task_follow = false;
        app.task_scroll = app.task_scroll.saturating_add(1);
      }
      KeyCode::PageUp => {
        app.task_follow = false;
        app.task_scroll = app.task_scroll.saturating_sub(10);
      }
      KeyCode::PageDown => {
        app.task_follow = false;
        app.task_scroll = app.task_scroll.saturating_add(10);
      }
      KeyCode::Home => {
        app.task_follow = false;
        app.task_scroll = 0;
      }
      KeyCode::End => {
        if let Some(live) = &app.task_live {
          app.task_scroll = crate::app::task_bottom_offset(&live.output());
        }
      }
      _ => {}
    }
    return;
  }

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

  if matches!(
    app.page(),
    crate::navigation::Page::Keybindings
      | crate::navigation::Page::KeybindingEdit
      | crate::navigation::Page::KeybindingCapture
  ) && app.has_keybinding_conflict()
  {
    match key.code {
      KeyCode::Char('r') => app.replace_keybinding_conflict(),
      KeyCode::Esc => app.cancel_keybinding_conflict(),
      _ => {}
    }
    return;
  }

  if matches!(
    app.page(),
    crate::navigation::Page::KeybindingEdit | crate::navigation::Page::KeybindingCapture
  ) && (app.is_keybinding_capturing() || key.code == KeyCode::Char('e'))
  {
    if !app.is_keybinding_capturing() {
      app.begin_keybinding_capture();
    } else {
      app.capture_keybinding(key);
    }
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

  if app.page() == crate::navigation::Page::MouseTouchpad {
    match key.code {
      KeyCode::Left | KeyCode::Char('h') => {
        let selected = app.navigation.current().selected;
        app.input_cycle(selected, -1);
      }
      KeyCode::Right | KeyCode::Char('l') => {
        let selected = app.navigation.current().selected;
        app.input_cycle(selected, 1);
      }
      KeyCode::Char(' ') => app.toggle_current(),
      _ => handle_regular_key(app, key),
    }
    return;
  }

  handle_regular_key(app, key);
}

fn handle_regular_key(app: &mut App, key: KeyEvent) {
  match key.code {
    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
    KeyCode::Tab => app.cycle_selection(1),
    KeyCode::BackTab => app.cycle_selection(-1),
    KeyCode::Right | KeyCode::Char('l') if app.on_buttons() => app.move_button(1),
    KeyCode::Left | KeyCode::Char('h') if app.on_buttons() => app.move_button(-1),
    KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => app.open_or_apply(),
    KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => app.back(),
    KeyCode::Char('/') => app.begin_search(),
    KeyCode::Char('r') => {
      if app.page() == crate::navigation::Page::System {
        app.refresh_system();
      } else {
        app.reset_current();
      }
    }
    KeyCode::Char(' ') => app.toggle_current(),
    KeyCode::Char('+') | KeyCode::Char('=') => app.adjust_size(1),
    KeyCode::Char('-') => app.adjust_size(-1),
    KeyCode::Home => {
      let selected = app.navigation.current().selected;
      app.move_selection(-(selected as isize));
    }
    KeyCode::End => {
      let count = app.item_count();
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
