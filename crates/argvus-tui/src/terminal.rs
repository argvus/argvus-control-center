use std::io::{self, Stdout};
use std::panic;

use crossterm::{
  cursor::{Hide, Show},
  event::{DisableBracketedPaste, EnableBracketedPaste},
  execute,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
  terminal: TuiTerminal,
}

impl TerminalGuard {
  pub fn new() -> io::Result<Self> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, Hide) {
      let _ = disable_raw_mode();
      return Err(error);
    }
    match Terminal::new(CrosstermBackend::new(stdout)) {
      Ok(terminal) => Ok(Self { terminal }),
      Err(error) => {
        restore();
        Err(error)
      }
    }
  }

  pub fn terminal_mut(&mut self) -> &mut TuiTerminal {
    &mut self.terminal
  }
}

impl Drop for TerminalGuard {
  fn drop(&mut self) {
    let _ = disable_raw_mode();
    let _ = execute!(
      self.terminal.backend_mut(),
      DisableBracketedPaste,
      Show,
      LeaveAlternateScreen
    );
    let _ = self.terminal.show_cursor();
  }
}

pub fn install_panic_hook() {
  let previous = panic::take_hook();
  panic::set_hook(Box::new(move |info| {
    restore();
    previous(info);
  }));
}

fn restore() {
  let _ = disable_raw_mode();
  let _ = execute!(
    io::stdout(),
    DisableBracketedPaste,
    Show,
    LeaveAlternateScreen
  );
}
