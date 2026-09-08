use anyhow::{Result, bail};
use argvus_control_center_about::Tab;

use crate::app::InitialRoute;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse_or_print() -> Result<Option<InitialRoute>> {
  let args: Vec<String> = std::env::args().skip(1).collect();
  match parse(&args) {
    Ok(Some(route)) => Ok(Some(route)),
    Ok(None) => Ok(None),
    Err(error) => {
      eprintln!("argvus-control-center: {error}\n");
      print_help();
      bail!(error)
    }
  }
}

pub fn parse(args: &[String]) -> Result<Option<InitialRoute>, String> {
  if args.is_empty() {
    return Ok(Some(InitialRoute::Home));
  }
  match args[0].as_str() {
    "-h" | "--help" if args.len() == 1 => {
      print_help();
      Ok(None)
    }
    "-V" | "--version" if args.len() == 1 => {
      println!("argvus-control-center {VERSION}");
      Ok(None)
    }
    "apps" | "default-apps" if args.len() == 1 => Ok(Some(InitialRoute::Apps)),
    "fonts" if args.len() == 1 => Ok(Some(InitialRoute::Fonts)),
    "about" if args.len() == 1 => Ok(Some(InitialRoute::About(Tab::System))),
    "about" if args.len() == 2 => {
      parse_about_tab(&args[1]).map(|tab| Some(InitialRoute::About(tab)))
    }
    value => Err(format!("unknown argument '{value}'")),
  }
}

fn parse_about_tab(value: &str) -> Result<Tab, String> {
  match value {
    "system" => Ok(Tab::System),
    "about" => Ok(Tab::About),
    "donate" => Ok(Tab::Donate),
    "credits" => Ok(Tab::Credits),
    "copyright" => Ok(Tab::Copyright),
    _ => Err(format!("unknown About tab '{value}'")),
  }
}

pub fn print_help() {
  println!(
    "argvus-control-center - keyboard-first ARGVUS control center\n\n\
     Usage:\n  argvus-control-center [apps|fonts|about [TAB]]\n\n\
     About tabs:\n  system, about, donate, credits, copyright\n\n\
     Options:\n  -h, --help       Print help\n  -V, --version    Print version\n\n\
     Global keys:\n  q                Quit\n  Esc              Back\n  ?                Contextual help"
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
  }

  #[test]
  fn parses_direct_routes() {
    assert_eq!(parse(&args(&["apps"])).unwrap(), Some(InitialRoute::Apps));
    assert_eq!(parse(&args(&["fonts"])).unwrap(), Some(InitialRoute::Fonts));
    assert_eq!(
      parse(&args(&["about", "credits"])).unwrap(),
      Some(InitialRoute::About(Tab::Credits))
    );
  }

  #[test]
  fn rejects_invalid_routes() {
    assert!(parse(&args(&["about", "unknown"])).is_err());
    assert!(parse(&args(&["fonts", "extra"])).is_err());
  }
}
