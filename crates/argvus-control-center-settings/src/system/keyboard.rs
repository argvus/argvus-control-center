use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::SettingsError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
  pub code: String,
  pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
  pub code: String,
  pub layout: String,
  pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct KeyboardInfo {
  pub x11_layout: String,
  pub x11_variant: String,
  pub x11_model: String,
  pub x11_options: String,
  pub console_keymap: String,
  pub hypr_layout: String,
  pub hypr_variant: String,
  pub hypr_options: String,
}

pub fn info() -> KeyboardInfo {
  let mut info =
    parse_localectl_status(&command_stdout("localectl", &["status"]).unwrap_or_default());
  let hypr = parse_hypr_keyboard_config();
  info.hypr_layout = hypr.0;
  info.hypr_variant = hypr.1;
  info.hypr_options = hypr.2;
  info
}

pub fn layouts() -> Vec<Layout> {
  parse_xkb_layouts(&read_xkb_rules().unwrap_or_default())
}

pub fn variants(layout: &str) -> Vec<Variant> {
  parse_xkb_variants(&read_xkb_rules().unwrap_or_default(), layout)
}

pub fn console_keymaps() -> Vec<String> {
  command_stdout("localectl", &["list-keymaps"])
    .unwrap_or_default()
    .lines()
    .map(str::trim)
    .filter(|line| is_keymap_name(line))
    .map(str::to_string)
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

pub fn set_x11_layout(layout: &str, valid: &[Layout]) -> Result<(), SettingsError> {
  if !valid.iter().any(|candidate| candidate.code == layout) {
    return Err(SettingsError::System(format!(
      "invalid keyboard layout: {layout}"
    )));
  }
  super::privileged::run(&["keyboard", "layout", layout])?;
  write_generated_hypr_input(Some(layout), None)?;
  apply_hypr_keyword("input:kb_layout", layout);
  restart_keyboard_daemon();
  Ok(())
}

pub fn set_x11_variant(
  layout: &str,
  variant: &str,
  valid: &[Variant],
) -> Result<(), SettingsError> {
  if !variant.is_empty() && !valid.iter().any(|candidate| candidate.code == variant) {
    return Err(SettingsError::System(format!(
      "invalid keyboard variant: {variant}"
    )));
  }
  super::privileged::run(&["keyboard", "variant", layout, variant])?;
  write_generated_hypr_input(Some(layout), Some(variant))?;
  apply_hypr_keyword("input:kb_variant", variant);
  restart_keyboard_daemon();
  Ok(())
}

pub fn set_console_keymap(keymap: &str, valid: &[String]) -> Result<(), SettingsError> {
  if !valid.iter().any(|candidate| candidate == keymap) {
    return Err(SettingsError::System(format!(
      "invalid console keymap: {keymap}"
    )));
  }
  super::privileged::run(&["keyboard", "console-keymap", keymap]).map(|_| ())
}

pub fn parse_localectl_status(contents: &str) -> KeyboardInfo {
  let mut info = KeyboardInfo::default();
  for line in contents.lines().map(str::trim) {
    let Some((key, value)) = line.split_once(':') else {
      continue;
    };
    let value = value.trim().to_string();
    match key {
      "VC Keymap" => info.console_keymap = value,
      "X11 Layout" => info.x11_layout = value,
      "X11 Model" => info.x11_model = value,
      "X11 Variant" => info.x11_variant = value,
      "X11 Options" => info.x11_options = value,
      _ => {}
    }
  }
  info
}

pub fn parse_xkb_layouts(contents: &str) -> Vec<Layout> {
  let mut in_layout = false;
  let mut out = Vec::new();
  for line in contents.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with('!') {
      in_layout = trimmed == "! layout";
      continue;
    }
    if !in_layout || trimmed.is_empty() {
      continue;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let Some(code) = parts.next() else {
      continue;
    };
    let description = parts.next().unwrap_or("").trim();
    if is_xkb_name(code) {
      out.push(Layout {
        code: code.to_string(),
        description: description.to_string(),
      });
    }
  }
  out
}

pub fn parse_xkb_variants(contents: &str, layout: &str) -> Vec<Variant> {
  let mut in_variant = false;
  let mut out = vec![Variant {
    code: String::new(),
    layout: layout.to_string(),
    description: "Default".to_string(),
  }];
  for line in contents.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with('!') {
      in_variant = trimmed == "! variant";
      continue;
    }
    if !in_variant || trimmed.is_empty() {
      continue;
    }
    let Some((left, description)) = trimmed.split_once(':') else {
      continue;
    };
    let mut parts = left.split_whitespace();
    let Some(code) = parts.next() else {
      continue;
    };
    let Some(entry_layout) = parts.next() else {
      continue;
    };
    if entry_layout == layout && is_xkb_name(code) {
      out.push(Variant {
        code: code.to_string(),
        layout: entry_layout.to_string(),
        description: description.trim().to_string(),
      });
    }
  }
  out
}

fn parse_hypr_keyboard_config() -> (String, String, String) {
  let generated = generated_hypr_input_path();
  let generated_contents = fs::read_to_string(generated).unwrap_or_default();
  let packaged = fs::read_to_string(hypr_config_path()).unwrap_or_default();
  let layout = extract_lua_string(&generated_contents, "kb_layout")
    .or_else(|| extract_lua_string(&packaged, "kb_layout"))
    .unwrap_or_default();
  let variant = extract_lua_string(&generated_contents, "kb_variant")
    .or_else(|| extract_lua_string(&packaged, "kb_variant"))
    .unwrap_or_default();
  let options = extract_lua_string(&generated_contents, "kb_options")
    .or_else(|| extract_lua_string(&packaged, "kb_options"))
    .unwrap_or_default();
  (layout, variant, options)
}

fn hypr_config_path() -> PathBuf {
  let user = argvus_control_center_core::paths::argvus_config_home().join("hypr/hyprland.lua");
  if user.exists() {
    user
  } else {
    argvus_control_center_core::paths::system_config_root().join("hypr/hyprland.lua")
  }
}

fn generated_hypr_input_path() -> PathBuf {
  argvus_control_center_core::paths::argvus_config_home().join("generated/hypr/input.lua")
}

fn extract_lua_string(contents: &str, key: &str) -> Option<String> {
  contents.lines().find_map(|line| {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(key)?.trim_start();
    let value = rest.strip_prefix('=')?.trim_start().trim_end_matches(',');
    value
      .strip_prefix('"')
      .and_then(|v| v.strip_suffix('"'))
      .map(str::to_string)
  })
}

fn write_generated_hypr_input(
  layout: Option<&str>,
  variant: Option<&str>,
) -> Result<(), SettingsError> {
  let current = parse_hypr_keyboard_config();
  let layout = layout.unwrap_or(&current.0);
  let variant = variant.unwrap_or(&current.1);
  let options = current.2;
  if !layout.split(',').all(is_xkb_name) {
    return Err(SettingsError::System(format!(
      "invalid Hyprland keyboard layout: {layout}"
    )));
  }
  if !variant.is_empty() && !variant.split(',').all(is_xkb_name) {
    return Err(SettingsError::System(format!(
      "invalid Hyprland keyboard variant: {variant}"
    )));
  }
  let path = generated_hypr_input_path();
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|error| SettingsError::System(error.to_string()))?;
  }
  let contents = format!(
    "-- Generated by argvus-control-center. Do not edit this file directly.\nreturn {{\n  kb_layout = \"{}\",\n  kb_variant = \"{}\",\n  kb_options = \"{}\",\n}}\n",
    lua_escape(layout),
    lua_escape(variant),
    lua_escape(&options)
  );
  let tmp = path.with_extension("lua.tmp");
  {
    let mut file =
      fs::File::create(&tmp).map_err(|error| SettingsError::System(error.to_string()))?;
    file
      .write_all(contents.as_bytes())
      .map_err(|error| SettingsError::System(error.to_string()))?;
    file
      .sync_all()
      .map_err(|error| SettingsError::System(error.to_string()))?;
  }
  fs::rename(&tmp, &path).map_err(|error| SettingsError::System(error.to_string()))
}

fn read_xkb_rules() -> Option<String> {
  [
    "/usr/share/X11/xkb/rules/base.lst",
    "/usr/share/X11/xkb/rules/evdev.lst",
  ]
  .into_iter()
  .map(Path::new)
  .find(|path| path.is_file())
  .and_then(|path| fs::read_to_string(path).ok())
}

pub fn is_xkb_name(value: &str) -> bool {
  !value.is_empty()
    && value
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

pub fn is_keymap_name(value: &str) -> bool {
  is_xkb_name(value)
    || value
      .bytes()
      .all(|b| matches!(b, b'/' | b'.') || b.is_ascii_alphanumeric() || b == b'-')
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
  let output = Command::new(command)
    .args(args)
    .stdin(Stdio::null())
    .output()
    .ok()?;
  output
    .status
    .success()
    .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn apply_hypr_keyword(key: &str, value: &str) {
  let _ = Command::new("hyprctl")
    .args(["keyword", key, value])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
}

fn restart_keyboard_daemon() {
  let _ = Command::new("argvus-sessionctl")
    .args(["restart", "keyboard-layout"])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
}

fn lua_escape(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_localectl_keyboard_status() {
    let parsed =
      parse_localectl_status("VC Keymap: br-abnt2\nX11 Layout: br\nX11 Variant: abnt2\n");
    assert_eq!(parsed.console_keymap, "br-abnt2");
    assert_eq!(parsed.x11_layout, "br");
    assert_eq!(parsed.x11_variant, "abnt2");
  }

  #[test]
  fn parses_xkb_layouts_and_variants() {
    let data = "! layout\n  br Portuguese (Brazil)\n  us English (US)\n! variant\n  abnt2 br: ABNT2\n  intl us: English intl\n";
    let layouts = parse_xkb_layouts(data);
    assert_eq!(layouts[0].code, "br");
    let variants = parse_xkb_variants(data, "br");
    assert!(variants.iter().any(|variant| variant.code == "abnt2"));
    assert!(!variants.iter().any(|variant| variant.code == "intl"));
  }

  #[test]
  fn rejects_invalid_xkb_names() {
    assert!(is_xkb_name("br"));
    assert!(is_xkb_name("br_abnt2"));
    assert!(!is_xkb_name("br;reboot"));
  }
}
