use std::time::Duration;

use anyhow::Result;
use argvus_control_center::app::App;
use argvus_control_center::{cli, event, ui};
use argvus_tui::terminal::{TerminalGuard, install_panic_hook};

fn main() -> Result<()> {
  let args: Vec<String> = std::env::args().skip(1).collect();
  if args.first().is_some_and(|arg| arg == "system-settings") {
    if let Err(error) = argvus_control_center_settings::system::command::run(&args[1..]) {
      eprintln!("argvus-control-center: {error}");
      std::process::exit(1);
    }
    return Ok(());
  }

  let Some(initial) = cli::parse_or_print()? else {
    return Ok(());
  };

  install_panic_hook();
  let mut app = App::new(initial);
  let mut terminal = TerminalGuard::new()?;
  let mut dirty = true;
  while !app.quit {
    if dirty {
      terminal
        .terminal_mut()
        .draw(|frame| ui::draw(&mut app, frame))?;
      dirty = false;
    }
    if crossterm::event::poll(Duration::from_millis(100))? {
      event::handle(&mut app, crossterm::event::read()?);
      dirty = true;
    }
    if app.settings.expire_status() {
      dirty = true;
    }
  }
  Ok(())
}
