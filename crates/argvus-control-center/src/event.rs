use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::{App, Route};

pub fn handle(app: &mut App, event: Event) {
  if let Event::Paste(text) = event {
    if app.route == Route::Settings {
      app.settings.admin_paste(&text);
    }
    return;
  }
  if let Event::Resize(width, height) = event {
    app.resize(width, height);
    return;
  }
  let Event::Key(key) = event else {
    return;
  };
  if key.kind != KeyEventKind::Press {
    return;
  }
  if app.route == Route::Settings && app.settings.admin.editor.is_some() {
    argvus_control_center_settings::event::handle(&mut app.settings, Event::Key(key));
    return;
  }
  if app.route == Route::Settings && app.settings.admin.busy() {
    return;
  }
  if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
    app.quit = true;
    return;
  }
  if app.help {
    if matches!(key.code, KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter) {
      app.help = false;
    }
    return;
  }
  if key.code == KeyCode::Char('?') {
    app.help = true;
    return;
  }
  if key.code == KeyCode::Char('q') {
    app.quit = true;
    return;
  }

  match app.route {
    Route::Home => handle_home(app, key),
    Route::Settings => {
      if key.code == KeyCode::Esc
        && !app.settings.searching
        && app.settings.error_modal.is_none()
        && app.settings.confirm.is_none()
        && !app.settings.hostname_editing
      {
        app.back();
      } else {
        argvus_control_center_settings::event::handle(&mut app.settings, Event::Key(key));
        app.lang = app.settings.lang;
        if app.settings.page() == argvus_control_center_settings::Page::Main {
          app.route = Route::Home;
        }
      }
    }
    Route::About => handle_about(app, key),
  }
}

fn handle_home(app: &mut App, key: KeyEvent) {
  match key.code {
    KeyCode::Up | KeyCode::Char('k') => app.move_home(-1),
    KeyCode::Down | KeyCode::Char('j') => app.move_home(1),
    KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => app.open_home(),
    _ => {}
  }
}

fn handle_about(app: &mut App, key: KeyEvent) {
  match key.code {
    KeyCode::Esc => app.back(),
    KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => app.about.prev_tab(),
    KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => app.about.next_tab(),
    KeyCode::Char('j') | KeyCode::Down => app.about.move_cursor(1),
    KeyCode::Char('k') | KeyCode::Up => app.about.move_cursor(-1),
    KeyCode::Enter => app.about.enter(),
    KeyCode::PageDown => app.about.page(1),
    KeyCode::PageUp => app.about.page(-1),
    KeyCode::Home => app.about.home(),
    KeyCode::End => app.about.end(),
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::{InitialRoute, Route};
  use argvus_control_center_about::Tab;
  use crossterm::event::{KeyEventState, KeyModifiers};

  fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new_with_kind_and_state(
      code,
      KeyModifiers::NONE,
      KeyEventKind::Press,
      KeyEventState::NONE,
    ))
  }

  #[test]
  fn escape_cancels_settings_confirmation_without_leaving_page() {
    let mut app = App::new(InitialRoute::Apps);
    app.settings.confirm = Some(argvus_control_center_settings::app::PendingAction::ResetApps);
    handle(&mut app, press(KeyCode::Esc));
    assert_eq!(app.route, Route::Settings);
    assert_eq!(
      app.settings.page(),
      argvus_control_center_settings::Page::DefaultApps
    );
    assert!(app.settings.confirm.is_none());
  }

  #[test]
  fn q_quits_from_about() {
    let mut app = App::new(InitialRoute::About(Tab::System));
    handle(&mut app, press(KeyCode::Char('q')));
    assert!(app.quit);
  }

  #[test]
  fn account_editor_receives_quit_and_help_characters() {
    let mut app = App::new(InitialRoute::Home);
    app.route = Route::Settings;
    app.settings.error_modal = None;
    app
      .settings
      .navigation
      .push(argvus_control_center_settings::Page::CreateUser);
    app.settings.normalize_selection();
    app.settings.open_or_apply();
    handle(&mut app, press(KeyCode::Char('q')));
    handle(&mut app, press(KeyCode::Char('?')));
    assert!(!app.quit);
    assert!(!app.help);
    assert!(app.settings.admin.editor.is_some());
    handle(&mut app, press(KeyCode::Esc));
    assert!(app.settings.admin.editor.is_none());
    assert_eq!(
      app.settings.page(),
      argvus_control_center_settings::Page::CreateUser
    );
  }

  #[test]
  fn q_quits_during_settings_search() {
    let mut app = App::new(InitialRoute::Fonts);
    app.settings.searching = true;
    handle(&mut app, press(KeyCode::Char('q')));
    assert!(app.quit);
  }

  #[test]
  fn escape_returns_from_about_to_home() {
    let mut app = App::new(InitialRoute::About(Tab::System));
    handle(&mut app, press(KeyCode::Esc));
    assert_eq!(app.route, Route::Home);
  }

  #[test]
  fn arrows_switch_about_tabs() {
    let mut app = App::new(InitialRoute::About(Tab::System));
    handle(&mut app, press(KeyCode::Right));
    assert_eq!(app.about.active_tab, Tab::About);
  }
}
