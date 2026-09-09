use anyhow::{Result, bail};
use argvus_control_center_about::Tab;
use argvus_control_center_settings::{Category, FontTarget, Page, SettingKind};

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
    "apps" | "default-apps" => parse_apps(&args[1..]).map(settings_route),
    "fonts" => parse_fonts(&args[1..]).map(settings_route),
    "locale" | "locale-region" => parse_locale(&args[1..]).map(settings_route),
    "language" if args.len() == 1 => Ok(settings_route(Page::Language)),
    "system" => parse_system(&args[1..]).map(settings_route),
    "about" if args.len() == 1 => Ok(Some(InitialRoute::About(Tab::System))),
    "about" if args.len() == 2 => {
      parse_about_tab(&args[1]).map(|tab| Some(InitialRoute::About(tab)))
    }
    value => Err(format!("unknown argument '{value}'")),
  }
}

fn settings_route(page: Page) -> Option<InitialRoute> {
  Some(InitialRoute::Settings(page))
}

fn option(value: &str) -> &str {
  value.strip_prefix("--").unwrap_or(value)
}

fn parse_apps(args: &[String]) -> Result<Page, String> {
  if args.is_empty() {
    return Ok(Page::DefaultApps);
  }
  if args.len() != 1 {
    return Err("apps accepts at most one page".to_string());
  }
  let category = match option(&args[0]) {
    "terminal" => Category::Terminal,
    "file-manager" => Category::FileManager,
    "text-editor" => Category::TextEditor,
    "terminal-editor" => Category::TerminalEditor,
    "browser" => Category::Browser,
    "image-viewer" => Category::ImageViewer,
    "pdf-viewer" => Category::PdfViewer,
    "video-player" => Category::VideoPlayer,
    "audio-player" => Category::AudioPlayer,
    "archive" => Category::Archive,
    "launcher" => Category::Launcher,
    value => return Err(format!("unknown Apps page '{value}'")),
  };
  Ok(Page::AppSelector(category))
}

fn parse_fonts(args: &[String]) -> Result<Page, String> {
  if args.is_empty() {
    return Ok(Page::Fonts);
  }
  if args.len() != 1 {
    return Err("fonts accepts at most one page".to_string());
  }
  let page = match option(&args[0]) {
    "taskbar" => Page::FontSelector(FontTarget::Taskbar),
    "widget-telemetry" | "desktop-telemetry" | "sysinfo" => Page::FontSelector(FontTarget::Sysinfo),
    "control-panel" => Page::FontSelector(FontTarget::ControlPanel),
    "system" => Page::FontSelector(FontTarget::System),
    "apps" => Page::FontSelector(FontTarget::Apps),
    "terminal" => Page::FontSelector(FontTarget::Terminal),
    "browser" => Page::FontSelector(FontTarget::Browser),
    "antialiasing" => Page::SettingSelector(SettingKind::Antialiasing),
    "hinting" => Page::SettingSelector(SettingKind::Hinting),
    "subpixel" => Page::SettingSelector(SettingKind::Subpixel),
    "dpi" => Page::SettingSelector(SettingKind::Dpi),
    value => return Err(format!("unknown Fonts page '{value}'")),
  };
  Ok(page)
}

fn parse_locale(args: &[String]) -> Result<Page, String> {
  if args.is_empty() {
    return Ok(Page::LocaleRegion);
  }
  if args.len() != 1 {
    return Err("locale accepts at most one page".to_string());
  }
  match option(&args[0]) {
    "timezone" => Ok(Page::TimeZone),
    "date-time" => Ok(Page::DateTime),
    "regional-locale" => Ok(Page::RegionalLocale),
    "system-locales" => Ok(Page::SystemLocales),
    "encoding" => Ok(Page::Encoding),
    "keyboard" => Ok(Page::Keyboard),
    "keyboard-layout" => Ok(Page::KeyboardLayout),
    "keyboard-variant" => Ok(Page::KeyboardVariant),
    "console-keymap" => Ok(Page::ConsoleKeymap),
    value => Err(format!("unknown Locale & Region page '{value}'")),
  }
}

fn parse_system(args: &[String]) -> Result<Page, String> {
  if args.is_empty() {
    return Ok(Page::System);
  }
  if args.len() != 1 {
    return Err("system accepts at most one page".to_string());
  }
  match option(&args[0]) {
    "hostname" => Ok(Page::Hostname),
    "firewall" => Ok(Page::Firewall),
    "users" => Ok(Page::Users),
    value => Err(format!("unknown System page '{value}'")),
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
     Usage:\n  argvus-control-center [apps|fonts|locale|language|system|about [TAB]]\n  argvus-control-center system-settings <domain> <action> [value...]\n\n\
     Page selectors:\n  apps [terminal|file-manager|text-editor|terminal-editor|browser|image-viewer|pdf-viewer|video-player|audio-player|archive|launcher]\n  fonts [taskbar|widget-telemetry|control-panel|system|apps|terminal|browser|antialiasing|hinting|subpixel|dpi]\n  locale [timezone|date-time|regional-locale|system-locales|encoding|keyboard|keyboard-layout|keyboard-variant|console-keymap]\n  system [hostname|firewall|users]\n\n\
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
    assert_eq!(
      parse(&args(&["apps"])).unwrap(),
      Some(InitialRoute::Settings(Page::DefaultApps))
    );
    assert_eq!(
      parse(&args(&["fonts"])).unwrap(),
      Some(InitialRoute::Settings(Page::Fonts))
    );
    assert_eq!(
      parse(&args(&["locale", "--keyboard"])).unwrap(),
      Some(InitialRoute::Settings(Page::Keyboard))
    );
    assert_eq!(
      parse(&args(&["system", "hostname"])).unwrap(),
      Some(InitialRoute::Settings(Page::Hostname))
    );
    assert_eq!(
      parse(&args(&["about", "credits"])).unwrap(),
      Some(InitialRoute::About(Tab::Credits))
    );
  }

  #[test]
  fn maps_every_settings_page() {
    let cases = [
      (
        vec!["apps", "terminal"],
        Page::AppSelector(Category::Terminal),
      ),
      (
        vec!["apps", "file-manager"],
        Page::AppSelector(Category::FileManager),
      ),
      (
        vec!["apps", "text-editor"],
        Page::AppSelector(Category::TextEditor),
      ),
      (
        vec!["apps", "terminal-editor"],
        Page::AppSelector(Category::TerminalEditor),
      ),
      (
        vec!["apps", "browser"],
        Page::AppSelector(Category::Browser),
      ),
      (
        vec!["apps", "image-viewer"],
        Page::AppSelector(Category::ImageViewer),
      ),
      (
        vec!["apps", "pdf-viewer"],
        Page::AppSelector(Category::PdfViewer),
      ),
      (
        vec!["apps", "video-player"],
        Page::AppSelector(Category::VideoPlayer),
      ),
      (
        vec!["apps", "audio-player"],
        Page::AppSelector(Category::AudioPlayer),
      ),
      (
        vec!["apps", "archive"],
        Page::AppSelector(Category::Archive),
      ),
      (
        vec!["apps", "launcher"],
        Page::AppSelector(Category::Launcher),
      ),
      (
        vec!["fonts", "taskbar"],
        Page::FontSelector(FontTarget::Taskbar),
      ),
      (
        vec!["fonts", "widget-telemetry"],
        Page::FontSelector(FontTarget::Sysinfo),
      ),
      (
        vec!["fonts", "control-panel"],
        Page::FontSelector(FontTarget::ControlPanel),
      ),
      (
        vec!["fonts", "system"],
        Page::FontSelector(FontTarget::System),
      ),
      (vec!["fonts", "apps"], Page::FontSelector(FontTarget::Apps)),
      (
        vec!["fonts", "terminal"],
        Page::FontSelector(FontTarget::Terminal),
      ),
      (
        vec!["fonts", "browser"],
        Page::FontSelector(FontTarget::Browser),
      ),
      (
        vec!["fonts", "antialiasing"],
        Page::SettingSelector(SettingKind::Antialiasing),
      ),
      (
        vec!["fonts", "hinting"],
        Page::SettingSelector(SettingKind::Hinting),
      ),
      (
        vec!["fonts", "subpixel"],
        Page::SettingSelector(SettingKind::Subpixel),
      ),
      (
        vec!["fonts", "dpi"],
        Page::SettingSelector(SettingKind::Dpi),
      ),
      (vec!["locale", "timezone"], Page::TimeZone),
      (vec!["locale", "date-time"], Page::DateTime),
      (vec!["locale", "regional-locale"], Page::RegionalLocale),
      (vec!["locale", "system-locales"], Page::SystemLocales),
      (vec!["locale", "encoding"], Page::Encoding),
      (vec!["locale", "keyboard"], Page::Keyboard),
      (vec!["locale", "keyboard-layout"], Page::KeyboardLayout),
      (vec!["locale", "keyboard-variant"], Page::KeyboardVariant),
      (vec!["locale", "console-keymap"], Page::ConsoleKeymap),
      (vec!["language"], Page::Language),
      (vec!["system"], Page::System),
      (vec!["system", "hostname"], Page::Hostname),
      (vec!["system", "firewall"], Page::Firewall),
      (vec!["system", "--users"], Page::Users),
    ];

    for (arguments, expected) in cases {
      assert_eq!(
        parse(&args(&arguments)).unwrap(),
        Some(InitialRoute::Settings(expected)),
        "arguments: {arguments:?}"
      );
    }
  }

  #[test]
  fn rejects_invalid_routes() {
    assert!(parse(&args(&["about", "unknown"])).is_err());
    assert!(parse(&args(&["fonts", "extra"])).is_err());
  }
}
