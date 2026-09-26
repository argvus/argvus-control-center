//! Implements keyboard-layout discovery and application in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::SettingsError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents `Layout`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Layout {
  pub code: String,
  pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents `Variant`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Variant {
  pub code: String,
  pub layout: String,
  pub description: String,
}

#[derive(Debug, Clone, Default)]
/// Represents `KeyboardInfo`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
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

/// Executes the `info` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn info() -> KeyboardInfo {
  let mut info =
    parse_localectl_status(&command_stdout("localectl", &["status"]).unwrap_or_default());
  let hypr = parse_hypr_keyboard_config();
  info.hypr_layout = hypr.0;
  info.hypr_variant = hypr.1;
  info.hypr_options = hypr.2;
  // Keyboard changes are session-scoped and intentionally do not update the
  // privileged localectl database.  The generated Hyprland input is therefore
  // the authoritative state shown by this page.
  if !info.hypr_layout.is_empty() {
    info.x11_layout = active_layout(&info.hypr_layout);
  }
  if !info.hypr_variant.is_empty() {
    info.x11_variant = first_csv_value(&info.hypr_variant);
  } else if !info.hypr_layout.is_empty() {
    info.x11_variant.clear();
  }
  info
}

/// Renders `layouts` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn layouts() -> Vec<Layout> {
  parse_xkb_layouts(&read_xkb_rules().unwrap_or_default())
}

/// Executes the `variants` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn variants(layout: &str) -> Vec<Variant> {
  parse_xkb_variants(&read_xkb_rules().unwrap_or_default(), layout)
}

/// Executes the `console_keymaps` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Applies the `set_x11_layout` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_x11_layout(layout: &str, valid: &[Layout]) -> Result<(), SettingsError> {
  if !valid.iter().any(|candidate| candidate.code == layout) {
    return Err(SettingsError::System(format!(
      "invalid keyboard layout: {layout}"
    )));
  }
  let layouts = merged_layout_list(&parse_hypr_keyboard_config().0, layout);
  let layouts = layouts.split(',').map(str::to_string).collect::<Vec<_>>();
  set_x11_layouts(layout, &layouts, valid)
}

/// Applies the `set_x11_layouts` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_x11_layouts(
  default_layout: &str,
  layouts: &[String],
  valid: &[Layout],
) -> Result<(), SettingsError> {
  if layouts.is_empty() || !layouts.iter().any(|layout| layout == default_layout) {
    return Err(SettingsError::System(
      "at least one keyboard layout must be selected".to_string(),
    ));
  }
  if layouts
    .iter()
    .any(|layout| !valid.iter().any(|candidate| candidate.code == *layout))
  {
    return Err(SettingsError::System(
      "invalid selected keyboard layout".to_string(),
    ));
  }
  let mut ordered = vec![default_layout.to_string()];
  ordered.extend(
    layouts
      .iter()
      .filter(|layout| layout.as_str() != default_layout)
      .cloned(),
  );
  let layouts = ordered.join(",");
  // A variant belongs to the selected layout.  Keeping (for example)
  // br/abnt2 while selecting us makes Hyprland reject the complete config.
  write_generated_hypr_input(Some(&layouts), Some(""))?;
  apply_hypr_keyword("input:kb_layout", &layouts);
  apply_hypr_keyword("input:kb_variant", "");
  switch_active_layout(default_layout, &layouts);
  reload_taskbar();
  Ok(())
}

/// Applies the `set_x11_variant` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_x11_variant(
  _layout: &str,
  variant: &str,
  valid: &[Variant],
) -> Result<(), SettingsError> {
  if !variant.is_empty() && !valid.iter().any(|candidate| candidate.code == variant) {
    return Err(SettingsError::System(format!(
      "invalid keyboard variant: {variant}"
    )));
  }
  // Preserve the complete cycle list while changing only the variant of the
  // currently selected layout.
  write_generated_hypr_input(None, Some(variant))?;
  apply_hypr_keyword("input:kb_variant", variant);
  reload_taskbar();
  Ok(())
}

/// Applies the `set_console_keymap` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_console_keymap(keymap: &str, valid: &[String]) -> Result<(), SettingsError> {
  if !valid.iter().any(|candidate| candidate == keymap) {
    return Err(SettingsError::System(format!(
      "invalid console keymap: {keymap}"
    )));
  }
  super::privileged::run(&["keyboard", "console-keymap", keymap]).map(|_| ())
}

/// Converts input data into `parse_localectl_status` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Converts input data into `parse_xkb_layouts` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Converts input data into `parse_xkb_variants` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Converts input data into `parse_hypr_keyboard_config` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_hypr_keyboard_config() -> (String, String, String) {
  if let Some(value) = canonical_keyboard_config() {
    return value;
  }
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

fn canonical_keyboard_config() -> Option<(String, String, String)> {
  let output = Command::new("argvus-config")
    .args(["get", "/hyprland/keyboard", "--raw"])
    .output()
    .ok()?;
  if !output.status.success() {
    return None;
  }
  let value: Value = serde_json::from_slice(&output.stdout).ok()?;
  Some((
    value.get("layout")?.as_str()?.to_string(),
    value
      .get("variant")
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_string(),
    value
      .get("options")
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_string(),
  ))
}

/// Executes the `hypr_config_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn hypr_config_path() -> PathBuf {
  let user = argvus_control_center_core::paths::config_home().join("hypr/hyprland.lua");
  if user.exists() {
    user
  } else {
    argvus_control_center_core::paths::system_config_root().join("hyprland/config/hyprland.lua")
  }
}

/// Executes the `generated_hypr_input_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn generated_hypr_input_path() -> PathBuf {
  argvus_control_center_core::paths::argvus_config_home().join("generated/hypr/input.lua")
}

/// Executes the `extract_lua_string` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Executes the `first_csv_value` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn first_csv_value(value: &str) -> String {
  value
    .split(',')
    .map(str::trim)
    .find(|part| !part.is_empty())
    .unwrap_or_default()
    .to_string()
}

/// Executes the `active_layout` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn active_layout(layouts: &str) -> String {
  let fallback = first_csv_value(layouts);
  let Some(output) = command_stdout("hyprctl", &["devices", "-j"]) else {
    return fallback;
  };
  let Ok(root) = serde_json::from_str::<Value>(&output) else {
    return fallback;
  };
  let Some(index) = root
    .get("keyboards")
    .and_then(Value::as_array)
    .and_then(|keyboards| {
      keyboards
        .iter()
        .find_map(|keyboard| keyboard.get("active_layout_index").and_then(Value::as_u64))
    })
  else {
    return fallback;
  };
  layouts
    .split(',')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .nth(index as usize)
    .unwrap_or(&fallback)
    .to_string()
}

/// Applies the `write_generated_hypr_input` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
  fs::rename(&tmp, &path).map_err(|error| SettingsError::System(error.to_string()))?;
  let value = serde_json::json!({
    "layout": layout,
    "variant": variant,
    "options": options,
  });
  match Command::new("argvus-config")
    .args(["set", "/hyprland/keyboard", &value.to_string()])
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .status()
  {
    Ok(status) if status.success() => Ok(()),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Ok(status) => Err(SettingsError::System(format!(
      "argvus-config exited with {status}"
    ))),
    Err(error) => Err(SettingsError::System(format!(
      "argvus-config unavailable: {error}"
    ))),
  }
}

/// Retrieves data for `read_xkb_rules` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Checks the condition represented by `is_xkb_name` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn is_xkb_name(value: &str) -> bool {
  !value.is_empty()
    && value
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// Checks the condition represented by `is_keymap_name` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn is_keymap_name(value: &str) -> bool {
  is_xkb_name(value)
    || value
      .bytes()
      .all(|b| matches!(b, b'/' | b'.') || b.is_ascii_alphanumeric() || b == b'-')
}

/// Executes the `command_stdout` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Applies the `apply_hypr_keyword` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn apply_hypr_keyword(key: &str, value: &str) {
  let _ = Command::new("hyprctl")
    .args(["keyword", key, value])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
}

/// Executes the `merged_layout_list` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn merged_layout_list(current: &str, layout: &str) -> String {
  let mut parts: Vec<String> = current
    .split(',')
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .map(str::to_string)
    .collect();
  let mut merged: Vec<String> = Vec::with_capacity(parts.len() + 1);
  for part in parts.drain(..) {
    if part != layout && !merged.contains(&part) {
      merged.push(part);
    }
  }
  merged.insert(0, layout.to_string());
  merged.join(",")
}

/// Executes the `switch_active_layout` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn switch_active_layout(layout: &str, layouts: &str) {
  let index = layouts
    .split(',')
    .position(|part| part == layout)
    .unwrap_or(0);
  let _ = Command::new("hyprctl")
    .args(["switchxkblayout", "all", &index.to_string()])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
}

/// Executes the `reload_taskbar` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn reload_taskbar() {
  // Keep this identical to SUPER+SHIFT+R, which is the supported ARGVUS
  // session reload and is required for the managed Waybar configuration.
  let _ = Command::new("argvus-sessionctl")
    .args(["reload"])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
}

/// Executes the `lua_escape` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn lua_escape(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  /// Converts input data into `parses_localectl_keyboard_status` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn parses_localectl_keyboard_status() {
    let parsed =
      parse_localectl_status("VC Keymap: br-abnt2\nX11 Layout: br\nX11 Variant: abnt2\n");
    assert_eq!(parsed.console_keymap, "br-abnt2");
    assert_eq!(parsed.x11_layout, "br");
    assert_eq!(parsed.x11_variant, "abnt2");
  }

  #[test]
  /// Converts input data into `parses_xkb_layouts_and_variants` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn parses_xkb_layouts_and_variants() {
    let data = "! layout\n  br Portuguese (Brazil)\n  us English (US)\n! variant\n  abnt2 br: ABNT2\n  intl us: English intl\n";
    let layouts = parse_xkb_layouts(data);
    assert_eq!(layouts[0].code, "br");
    let variants = parse_xkb_variants(data, "br");
    assert!(variants.iter().any(|variant| variant.code == "abnt2"));
    assert!(!variants.iter().any(|variant| variant.code == "intl"));
  }

  #[test]
  /// Executes the `rejects_invalid_xkb_names` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn rejects_invalid_xkb_names() {
    assert!(is_xkb_name("br"));
    assert!(is_xkb_name("br_abnt2"));
    assert!(!is_xkb_name("br;reboot"));
  }

  #[test]
  /// Executes the `merged_layout_list_moves_the_chosen_layout_first` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn merged_layout_list_moves_the_chosen_layout_first() {
    assert_eq!(merged_layout_list("", "br"), "br");
    assert_eq!(merged_layout_list("br", "br"), "br");
    assert_eq!(merged_layout_list("br,us", "us"), "us,br");
    assert_eq!(merged_layout_list("us,br,de", "br"), "br,us,de");
    assert_eq!(merged_layout_list("us,br", "us"), "us,br");
  }

  #[test]
  /// Executes the `first_csv_value_reads_the_active_layout` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn first_csv_value_reads_the_active_layout() {
    assert_eq!(first_csv_value("us,br"), "us");
    assert_eq!(first_csv_value("  br  "), "br");
    assert_eq!(first_csv_value(""), "");
  }

  #[test]
  /// Executes the `active_layout_falls_back_to_the_first_configured_layout` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn active_layout_falls_back_to_the_first_configured_layout() {
    assert_eq!(active_layout("br,us"), "br");
  }
}
