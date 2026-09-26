//! Implements isolated integration with system tools and APIs in crate `argvus control center appearance`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use crate::model::{
  AppearanceState, ControlPanelCard, ControlPanelCards, EffectSurface, TaskbarPosition,
  TaskbarUtilityGroupMode, WallpaperCollection, WallpaperEntry, WallpaperMode,
  WidgetTelemetryBlock, WidgetTelemetryBlocks, canonical_theme_id, normalize_hex_color,
};
use argvus_control_center_core::{
  paths::{argvus_config_home, cache_home, system_config_root},
  process::{ProcessError, ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub use crate::profile::{
  active_custom_theme, apply_custom_theme, custom_themes, delete_custom_theme,
  export_named as export_theme_profile, import_archive as import_theme_profile,
  inspect_archive as inspect_theme_profile, list_import_archives,
  preview_export_path as preview_theme_profile_path,
};

/// Defines the constant `WALLPAPERS_DIR`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const WALLPAPERS_DIR: &str = "/usr/share/backgrounds/argvus";
/// Defines the constant `DEFAULT_THEME`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const DEFAULT_THEME: &str = "argvus-dark";
/// Defines the constant `DEFAULT_ACCENT`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const DEFAULT_ACCENT: &str = "#3590bd";

/// Executes the `script` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn script(name: &str) -> PathBuf {
  let project = match name {
    "effects-toggle.sh" => "session",
    "theme-switch.sh"
    | "accent-switch.sh"
    | "hypr-wallpaper-pick.sh"
    | "taskbar-right-2-mode.sh"
    | "brightness-switch.sh" => "appearance",
    "bluetooth-control.sh" => "network",
    "hyprlock-theme.sh" => "lock",
    "spaces-switch.sh" | "borders-switch.sh" => "hyprland",
    _ => "session",
  };
  system_config_root().join(project).join("sh").join(name)
}

/// Returns the Control Panel-owned preference helper without duplicating its
/// persistence rules in the Control Center.
fn control_panel_cards_script() -> PathBuf {
  system_config_root()
    .join("control-panel")
    .join("sh")
    .join("cards-config.sh")
}

/// Executes the `run_script` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn run_script(script_path: &Path, args: &[&str]) -> Result<(), String> {
  run_script_with_env(script_path, args, &[])
}

fn run_script_with_env(
  script_path: &Path,
  args: &[&str],
  environment: &[(&str, &str)],
) -> Result<(), String> {
  if !script_path.is_file() {
    return Err(format!("script não encontrado: {}", script_path.display()));
  }
  let mut request = ProcessRequest::new("sh").arg(script_path.to_string_lossy().to_string());
  for argument in args {
    if argument.is_empty()
      || argument
        .chars()
        .any(|character| character.is_control() || character == '\n')
    {
      return Err("invalid script argument".into());
    }
    request = request.arg(*argument);
  }
  for (name, value) in environment {
    request = request.env(*name, *value);
  }
  let output = SystemProcessRunner
    .run(&request)
    .map_err(|error| error.to_string())?;
  if output.status.is_none_or(|status| status != 0) {
    let stderr = terminal_text(&String::from_utf8_lossy(&output.stderr))
      .trim()
      .to_string();
    return Err(if stderr.is_empty() {
      format!("{} falhou", script_path.display())
    } else {
      stderr
    });
  }
  Ok(())
}

pub(crate) fn run_profile_script_with_env(
  name: &str,
  args: &[&str],
  environment: &[(&str, &str)],
) -> Result<(), String> {
  if name == "argvus-widget-telemetry-toggle" {
    let mut request = ProcessRequest::new(name);
    for argument in args {
      request = request.arg(*argument);
    }
    for (name, value) in environment {
      request = request.env(*name, *value);
    }
    let output = SystemProcessRunner
      .run(&request)
      .map_err(|error| error.to_string())?;
    return (output.status == Some(0))
      .then_some(())
      .ok_or_else(|| "telemetry apply failed".into());
  }
  let path = script(name);
  run_script_with_env(&path, args, environment)
}

/// Executes the `run_script_output` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn run_script_output(script_path: &Path, args: &[&str]) -> Option<String> {
  if !script_path.is_file() {
    return None;
  }
  let mut request = ProcessRequest::new("sh").arg(script_path.to_string_lossy().to_string());
  for argument in args {
    if argument.is_empty()
      || argument
        .chars()
        .any(|character| character.is_control() || character == '\n')
    {
      return None;
    }
    request = request.arg(*argument);
  }
  let output = SystemProcessRunner
    .run(&request.timeout(Duration::from_secs(5)))
    .ok()?;
  if output.status.is_none_or(|status| status != 0) {
    return None;
  }
  Some(terminal_text(&String::from_utf8_lossy(&output.stdout)))
}

/// Runs the canonical configuration CLI without introducing a second
/// configuration parser into the Control Center. Missing binaries are treated
/// as a compatibility condition for older installations; installed binaries
/// remain authoritative and report validation or persistence failures.
fn run_canonical_config_command(args: &[&str]) -> Result<Option<String>, String> {
  let argvus_config_path = argvus_config_home();
  let config_home = argvus_config_path
    .parent()
    .ok_or_else(|| "invalid ARGVUS configuration path".to_string())?;
  let mut request = ProcessRequest::new("argvus-config")
    .env(
      "ARGVUS_CONFIG_HOME",
      config_home.to_string_lossy().to_string(),
    )
    .timeout(Duration::from_secs(5));
  for argument in args {
    request = request.arg(*argument);
  }
  let output = match SystemProcessRunner.run(&request) {
    Ok(output) => output,
    Err(ProcessError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
      return Ok(None);
    }
    Err(error) => return Err(error.to_string()),
  };
  if output.timed_out || output.status != Some(0) {
    let stderr = terminal_text(&String::from_utf8_lossy(&output.stderr))
      .trim()
      .to_string();
    return Err(if stderr.is_empty() {
      "argvus-config falhou".into()
    } else {
      stderr
    });
  }
  Ok(Some(terminal_text(&String::from_utf8_lossy(
    &output.stdout,
  ))))
}

fn canonical_config_document() -> Option<Value> {
  let output = run_canonical_config_command(&["export", "--scope", "appearance"])
    .ok()
    .flatten()?;
  serde_json::from_str(&output).ok()
}

fn canonical_config_value<'a>(document: &'a Value, pointer: &str) -> Option<&'a Value> {
  document.pointer(pointer)
}

fn canonical_config_string(document: &Value, pointer: &str) -> Option<String> {
  canonical_config_value(document, pointer)
    .and_then(Value::as_str)
    .map(str::to_owned)
}

fn canonical_config_bool(document: &Value, pointer: &str) -> Option<bool> {
  canonical_config_value(document, pointer).and_then(Value::as_bool)
}

fn canonical_config_integer(document: &Value, pointer: &str) -> Option<i32> {
  canonical_config_value(document, pointer)
    .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
    .and_then(|value| i32::try_from(value).ok())
}

fn canonical_config_cards(document: &Value, mut cards: ControlPanelCards) -> ControlPanelCards {
  let Some(values) = canonical_config_value(document, "/control_panel/cards") else {
    return cards;
  };
  let entries = values
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|entry| Some((entry.get("id")?.as_str()?, entry.get("enabled")?.as_bool()?)))
    .chain(
      values
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| Some((key.as_str(), value.as_bool()?))),
    );
  for (key, enabled) in entries {
    let Some(card) = [
      ControlPanelCard::User,
      ControlPanelCard::Notifications,
      ControlPanelCard::Calendar,
      ControlPanelCard::Weather,
      ControlPanelCard::Volume,
      ControlPanelCard::Brightness,
      ControlPanelCard::Network,
      ControlPanelCard::Bluetooth,
      ControlPanelCard::System,
      ControlPanelCard::Appearance,
      ControlPanelCard::Session,
      ControlPanelCard::Display,
      ControlPanelCard::SpacesBordersPosition,
      ControlPanelCard::Power,
    ]
    .into_iter()
    .find(|card| card.key() == key) else {
      continue;
    };
    cards.set(card, enabled);
  }
  cards
}

fn control_panel_cards_value(cards: &ControlPanelCards) -> Value {
  Value::Array(
    ControlPanelCard::ALL
      .into_iter()
      .map(|card| {
        serde_json::json!({
          "id": card.key(),
          "enabled": cards.enabled(card),
        })
      })
      .collect(),
  )
}

fn canonical_widget_telemetry_blocks(document: &Value) -> WidgetTelemetryBlocks {
  let Some(values) = canonical_config_value(document, "/control_panel/widget_telemetry_blocks")
    .and_then(Value::as_array)
  else {
    return WidgetTelemetryBlocks::default();
  };
  let mut blocks = WidgetTelemetryBlocks::default();
  for block in WidgetTelemetryBlock::ALL {
    blocks.set(block, false);
  }
  for value in values.iter().filter_map(Value::as_str) {
    for block in WidgetTelemetryBlock::ALL {
      if block.key() == value {
        blocks.set(block, true);
      }
    }
  }
  blocks
}

fn persist_canonical_config_value(pointer: &str, value: Value) -> Result<(), String> {
  let serialized = serde_json::to_string(&value).map_err(|error| error.to_string())?;
  let _ = run_canonical_config_command(&["set", pointer, &serialized])?;
  Ok(())
}

/// Executes the `command_words` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn command_words(command: &str) -> Vec<String> {
  command
    .split_whitespace()
    .map(str::to_string)
    .filter(|word| !word.is_empty())
    .collect()
}

/// Checks the condition represented by `is_tui_file_manager` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn is_tui_file_manager(command: &[String]) -> bool {
  matches!(
    command.first().map(String::as_str),
    Some(
      "argvus"
        | "spf"
        | "superfile"
        | "yazi"
        | "ranger"
        | "lf"
        | "joshuto"
        | "broot"
        | "mc"
        | "nnn"
    )
  )
}

/// Executes the `default_app` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn default_app(category: &str) -> Option<Vec<String>> {
  let output = run_script_output(
    &system_config_root().join("session/sh/get-default.sh"),
    &[category],
  )?;
  let words = command_words(output.trim());
  (!words.is_empty()).then_some(words)
}

/// Runs a Hyprland Lua `dispatch` and reports whether it was accepted. Places
/// that dropped a dispatcher (e.g. `window not found`) still exit 0, logging a
/// `warning:`/`error:` on stderr, so only stderr-clean runs count as success.
fn hyprctl_lua_dispatch(expression: &str) -> bool {
  let Ok(output) = SystemProcessRunner.run(
    &ProcessRequest::new("hyprctl")
      .arg("dispatch")
      .arg(expression),
  ) else {
    return false;
  };
  if output.timed_out || output.status != Some(0) {
    return false;
  }
  let stderr = String::from_utf8_lossy(&output.stderr);
  !stderr.contains("warning:") && !stderr.contains("error:")
}

/// Focuses a window (by address or class) and raises it to the front, above the
/// Control Center window that launched it.
fn focus_and_raise(target: &str) {
  // Only raise once the target is actually focused; otherwise the dispatcher
  // below would raise whatever window currently has focus (the Control Center).
  if hyprctl_lua_dispatch(&format!("hl.dsp.focus({{ window = '{target}' }})")) {
    let _ = hyprctl_lua_dispatch("hl.dsp.window.bring_to_top({})");
  }
}

/// Terminal-backed wallpaper chooser: opens as a floating, centered window in
/// the standard 1024x768 File Manager proportion, on top of the Control Center.
fn focus_wallpaper_picker(child_id: u32) {
  // `argvus-tui-terminal` `exec`s the terminal, so `child_id` is also the PID
  // of the Wayland surface. Cold terminal startup can take a few seconds, so
  // poll patiently; only float/center/raise a window that actually exists, to
  // avoid operating on whatever has focus meanwhile (the Control Center).
  for _ in 0..80 {
    thread::sleep(Duration::from_millis(100));
    let Some(address) = client_address_for_pid(child_id) else {
      continue;
    };
    if hyprctl_lua_dispatch(&format!("hl.dsp.focus({{ window = 'address:{address}' }})")) {
      let _ = hyprctl_lua_dispatch("hl.dsp.window.float({ action = 'enable' })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.resize({ x = 1024, y = 768 })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.center({})");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.bring_to_top({})");
    }
    return;
  }
  // Fallback for wrappers that fork instead of exec: match the known class.
  for _ in 0..40 {
    thread::sleep(Duration::from_millis(100));
    if hyprctl_lua_dispatch("hl.dsp.focus({ window = 'class:argvus-wallpaper-picker' })") {
      let _ = hyprctl_lua_dispatch("hl.dsp.window.float({ action = 'enable' })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.resize({ x = 1024, y = 768 })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.center({})");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.bring_to_top({})");
      break;
    }
  }
}

/// Best-effort Wayland class (`class:`) used by the GUI File Managers Argvus
/// knows about. TUI managers are matched by their own `argvus-wallpaper-picker`
/// terminal window instead.
fn gui_file_manager_class(command: &str) -> Option<&'static str> {
  let name = Path::new(command)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(command);
  match name {
    "nautilus" => Some("org.gnome.Nautilus"),
    "nemo" => Some("org.nemo.Nemo"),
    "thunar" => Some("Thunar"),
    "dolphin" => Some("org.kde.dolphin"),
    "pcmanfm" | "pcmanfm-qt" => Some("pcmanfm"),
    "krusader" => Some("org.kde.krusader"),
    "caja" => Some("org.mate.Caja"),
    "doublecmd" => Some("doublecmd"),
    _ => None,
  }
}

/// Resolves the Hyprland client address (`0x…`) mapped to `pid`, if the
/// window already exists.
fn client_address_for_pid(pid: u32) -> Option<String> {
  let output = SystemProcessRunner
    .run(&ProcessRequest::new("hyprctl").arg("-j").arg("clients"))
    .ok()?;
  if output.status.is_none_or(|status| status != 0) {
    return None;
  }
  let value: Value = serde_json::from_slice(&output.stdout).ok()?;
  value
    .as_array()?
    .iter()
    .find(|client| client.get("pid").and_then(Value::as_u64) == Some(pid as u64))
    .and_then(|client| client.get("address").and_then(Value::as_str))
    .map(str::to_string)
}

/// Brings a freshly launched GUI File Manager window to the front, focused, on
/// top of the `argvus-control-center` window. The Wayland surface appears
/// asynchronously, so the exact process is polled first; GTK/KDE apps may be
/// D-Bus activated, in which case the known class is used as a fallback.
fn raise_file_manager_window(pid: u32, class: Option<&str>) {
  for _ in 0..80 {
    thread::sleep(Duration::from_millis(100));
    if let Some(address) = client_address_for_pid(pid) {
      focus_and_raise(&format!("address:{address}"));
      return;
    }
  }
  if let Some(class) = class {
    let target = format!("class:{class}");
    for _ in 0..40 {
      thread::sleep(Duration::from_millis(100));
      focus_and_raise(&target);
    }
  }
}

/// Retrieves data for `read_first` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn read_first(path: &Path, fallback: &str) -> String {
  fs::read_to_string(path)
    .ok()
    .and_then(|content| content.lines().next().map(str::to_string))
    .filter(|line| !line.trim().is_empty())
    .unwrap_or_else(|| fallback.to_string())
}

/// Retrieves data for `read_first_or_default` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn read_first_or_default(path: &Path) -> Option<String> {
  fs::read_to_string(path)
    .ok()
    .and_then(|content| content.lines().next().map(str::to_string))
    .map(|line| line.trim().to_string())
}

/// Executes the `active_theme_file` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn active_theme_file() -> PathBuf {
  argvus_config_home().join(".active-theme")
}

/// Executes the `accent_file` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn accent_file() -> PathBuf {
  argvus_config_home().join(".accent-color")
}

/// Executes the `theme_default_accent` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn theme_default_accent(theme: &str) -> Option<&'static str> {
  match theme {
    "dracula" | "dracula-float" => Some("#BD93F9"),
    "argvus-dark" | "argvus-dark-float" => Some("#3590bd"),
    "silver-dark" | "silver-dark-float" => Some("#595959"),
    "argvus-light" | "argvus-light-float" => Some("#181818"),
    "github-light" | "github-light-float" => Some("#0969DA"),
    "one-light" | "one-light-float" => Some("#4078F2"),
    "everforest-light" | "everforest-light-float" => Some("#3A94C5"),
    "solarized-light" | "solarized-light-float" => Some("#268BD2"),
    "rose-pine" | "rose-pine-float" => Some("#C4A7E7"),
    "frost" | "frost-float" => Some("#0969DA"),
    "catppuccin-latte" | "catppuccin-latte-float" => Some("#1E66F5"),
    "gruvbox-light" | "gruvbox-light-float" => Some("#458588"),
    "slate-dark" | "slate-dark-float" => Some("#7391a5"),
    "universe" | "universe-float" => Some("#eeeeee"),
    "gruvbox-high-dark" | "gruvbox-high-dark-float" => Some("#D79921"),
    "gruvbox-dark" | "gruvbox-dark-float" => Some("#D4BE98"),
    "tokyo-night" | "tokyo-night-float" => Some("#7AA2F7"),
    "solitude" | "solitude-float" => Some("#798186"),
    "sunset" | "sunset-float" => Some("#E2BE8A"),
    "hackerman" | "hackerman-float" => Some("#82FB9C"),
    "monokai-dark" | "monokai-dark-float" => Some("#78DCE8"),
    _ => None,
  }
}

/// Converts input data into `parse_pairs` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_pairs(output: &str) -> impl Iterator<Item = (&str, &str)> {
  output.lines().filter_map(|line| {
    line
      .split_once('=')
      .map(|(key, value)| (key.trim(), value.trim()))
  })
}

/// Converts input data into `parse_spacing_status` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_spacing_status(output: &str, state: &mut AppearanceState) {
  for (key, value) in parse_pairs(output) {
    match key {
      "waybar_pos" => state.waybar_pos = TaskbarPosition::from_value(value),
      "waybar_top" => state.waybar_top = value.parse().unwrap_or(state.waybar_top),
      "waybar_left" => state.waybar_left = value.parse().unwrap_or(state.waybar_left),
      "waybar_right" => state.waybar_right = value.parse().unwrap_or(state.waybar_right),
      "waybar_bottom" => state.waybar_bottom = value.parse().unwrap_or(state.waybar_bottom),
      "gaps_in" => state.gaps_in = value.parse().unwrap_or(state.gaps_in),
      "gaps_out_top" => state.gaps_out_top = value.parse().unwrap_or(state.gaps_out_top),
      "gaps_out_left" => state.gaps_out_left = value.parse().unwrap_or(state.gaps_out_left),
      "gaps_out_right" => state.gaps_out_right = value.parse().unwrap_or(state.gaps_out_right),
      "gaps_out_bottom" => state.gaps_out_bottom = value.parse().unwrap_or(state.gaps_out_bottom),
      _ => {}
    }
  }
}

/// Converts input data into `parse_borders_status` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_borders_status(output: &str, state: &mut AppearanceState) {
  for (key, value) in parse_pairs(output) {
    match key {
      "rounded" => state.rounded = value == "1",
      "rounding" => state.rounding = value.parse().unwrap_or(state.rounding),
      "thickness" => state.thickness = value.parse().unwrap_or(state.thickness),
      _ => {}
    }
  }
}

/// Reads one independent visual state from the shared session contract.
fn effect_state(component: &str) -> bool {
  let path = argvus_config_home().join("state").join(component);
  match read_first_or_default(&path).as_deref() {
    Some("enabled") => true,
    Some("disabled") => false,
    _ => matches!(
      run_script_output(&script("effects-toggle.sh"), &[component, "status"])
        .as_deref()
        .map(str::trim),
      Some("enabled")
    ),
  }
}

fn effect_value(kind: &str, surface: EffectSurface) -> i32 {
  run_script_output(
    &script("effects-toggle.sh"),
    &[&format!("{kind}-value"), surface.key(), "get"],
  )
  .and_then(|value| value.trim().parse::<i32>().ok())
  .filter(|value| (0..=100).contains(value))
  .unwrap_or(50)
}

fn effect_enabled(kind: &str, surface: EffectSurface) -> bool {
  run_script_output(
    &script("effects-toggle.sh"),
    &["effect-enabled", surface.key(), kind],
  )
  .is_some_and(|value| value.trim() == "enabled")
}

fn control_panel_enabled() -> bool {
  run_script_output(&control_panel_cards_script(), &["master", "status"])
    .is_none_or(|value| value.trim() != "disabled")
}

/// Executes the `telemetry_state` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn telemetry_state() -> bool {
  if let Ok(output) = SystemProcessRunner.run(
    &ProcessRequest::new("env")
      .arg("ARGVUS_MACHINE_OUTPUT=1")
      .arg("argvus-widget-telemetry-toggle")
      .arg("status")
      .timeout(Duration::from_secs(3)),
  ) && output.status.is_none_or(|status| status == 0)
  {
    match terminal_text(&String::from_utf8_lossy(&output.stdout)).trim() {
      "enabled" => return true,
      "disabled" => return false,
      _ => {}
    }
  }
  let cache = cache_home().join("argvus").join("waybar");
  let candidates = [
    cache.join("widget-telemetry-state"),
    cache.join("desktop-telemetry-state"),
    cache.join("sysinfo-state"),
  ];
  for candidate in candidates {
    match read_first_or_default(&candidate).as_deref() {
      Some("enabled") => return true,
      Some("disabled") => return false,
      _ => {}
    }
  }
  false
}

/// Reads the widget-owned machine format while retaining enabled defaults when
/// an older package does not yet expose block preferences.
fn telemetry_blocks() -> WidgetTelemetryBlocks {
  let mut blocks = WidgetTelemetryBlocks::default();
  let Ok(output) = SystemProcessRunner.run(
    &ProcessRequest::new("env")
      .arg("ARGVUS_MACHINE_OUTPUT=1")
      .arg("argvus-widget-telemetry-toggle")
      .arg("blocks")
      .arg("status")
      .timeout(Duration::from_secs(3)),
  ) else {
    return blocks;
  };
  if output.status.is_some_and(|status| status != 0) {
    return blocks;
  }
  for line in terminal_text(&String::from_utf8_lossy(&output.stdout)).lines() {
    let Some((key, value)) = line.split_once('=') else {
      continue;
    };
    let block = match key.trim() {
      "system" => WidgetTelemetryBlock::System,
      "cpu_gpu" => WidgetTelemetryBlock::CpuGpu,
      "memory" => WidgetTelemetryBlock::Memory,
      "storage" => WidgetTelemetryBlock::Storage,
      "processes" => WidgetTelemetryBlock::Processes,
      "network" => WidgetTelemetryBlock::Network,
      "keys" => WidgetTelemetryBlock::Shortcuts,
      _ => continue,
    };
    match value.trim() {
      "enabled" => blocks.set(block, true),
      "disabled" => blocks.set(block, false),
      _ => {}
    }
  }
  blocks
}

/// Decodes the Control Panel helper status and keeps package defaults when an
/// older installation does not provide the helper or returns malformed data.
fn control_panel_cards() -> ControlPanelCards {
  let mut cards = run_script_output(&control_panel_cards_script(), &["status"])
    .as_deref()
    .map(parse_control_panel_cards)
    .unwrap_or_default();
  // Keep the Control Center list identical to the cards' own runtime checks.
  // These probes are intentionally independent: a disabled card remains disabled
  // when hardware is temporarily disconnected and becomes available again later.
  // Hardware probes can invoke external tools (including short timeouts), so
  // run them concurrently on the worker thread instead of serializing them.
  let (bluetooth_available, brightness_available) = std::thread::scope(|scope| {
    let bluetooth = scope.spawn(|| {
      run_script_output(&script("bluetooth-control.sh"), &["status"])
        .is_some_and(|status| status.lines().any(|line| line.trim() == "available=yes"))
    });
    let brightness = scope.spawn(|| {
      run_script_output(&script("brightness-switch.sh"), &["--status"])
        .is_some_and(|backend| matches!(backend.trim(), "brightnessctl" | "ddcutil"))
    });
    (
      bluetooth.join().unwrap_or(false),
      brightness.join().unwrap_or(false),
    )
  });
  cards.set_available(ControlPanelCard::Bluetooth, bluetooth_available);
  cards.set_available(ControlPanelCard::Brightness, brightness_available);
  cards
}

/// Parses helper JSON independently from process execution for deterministic
/// coverage of compatibility and malformed-status fallbacks.
fn parse_control_panel_cards(output: &str) -> ControlPanelCards {
  let mut cards = ControlPanelCards::default();
  let Ok(value) = serde_json::from_str::<Value>(output) else {
    return cards;
  };
  let Some(entries) = value.get("cards").and_then(Value::as_array) else {
    return cards;
  };
  for entry in entries {
    let Some(key) = entry.get("id").and_then(Value::as_str) else {
      continue;
    };
    let Some(enabled) = entry.get("enabled").and_then(Value::as_bool) else {
      continue;
    };
    let card = match key {
      "user" => ControlPanelCard::User,
      "notifications" => ControlPanelCard::Notifications,
      "calendar" => ControlPanelCard::Calendar,
      "weather" => ControlPanelCard::Weather,
      "volume" => ControlPanelCard::Volume,
      "brightness" => ControlPanelCard::Brightness,
      "network" => ControlPanelCard::Network,
      "bluetooth" => ControlPanelCard::Bluetooth,
      "system" => ControlPanelCard::System,
      "appearance" => ControlPanelCard::Appearance,
      "session" => ControlPanelCard::Session,
      "display" => ControlPanelCard::Display,
      "spaces-borders-position" => ControlPanelCard::SpacesBordersPosition,
      "power" => ControlPanelCard::Power,
      _ => continue,
    };
    cards.set(card, enabled);
  }
  cards
}

/// Executes the `list_wallpapers` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn classify_wallpaper(relative: &Path) -> Option<(WallpaperCollection, WallpaperMode)> {
  let components = relative
    .components()
    .filter_map(|component| component.as_os_str().to_str())
    .collect::<Vec<_>>();
  match components.as_slice() {
    ["argvus-dark.jxl"] => Some((WallpaperCollection::Abstract, WallpaperMode::Dark)),
    ["argvus-light.jxl"] => Some((WallpaperCollection::Abstract, WallpaperMode::Light)),
    [collection, mode, filename] if filename.ends_with(".jxl") => {
      let collection = match *collection {
        "abstract" => WallpaperCollection::Abstract,
        "landscape" => WallpaperCollection::Landscape,
        _ => return None,
      };
      let mode = match *mode {
        "dark" => WallpaperMode::Dark,
        "light" => WallpaperMode::Light,
        _ => return None,
      };
      Some((collection, mode))
    }
    _ => None,
  }
}

pub fn list_wallpapers() -> Vec<WallpaperEntry> {
  fn collect(root: &Path, current: &Path, entries: &mut Vec<WallpaperEntry>) {
    let Ok(directory_entries) = fs::read_dir(current) else {
      return;
    };
    for entry in directory_entries.flatten() {
      let path = entry.path();
      if path.is_dir() {
        collect(root, &path, entries);
      } else if path.extension().is_some_and(|extension| extension == "jxl")
        && let Ok(relative) = path.strip_prefix(root)
        && let Some((collection, mode)) = classify_wallpaper(relative)
      {
        entries.push(WallpaperEntry {
          path: relative.to_string_lossy().into_owned(),
          collection,
          mode,
        });
      }
    }
  }

  let mut entries = Vec::new();
  collect(
    Path::new(WALLPAPERS_DIR),
    Path::new(WALLPAPERS_DIR),
    &mut entries,
  );
  entries.sort_by(|left, right| left.path.cmp(&right.path));
  entries
}

/// Executes the `hyprpaper_config_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn hyprpaper_config_path() -> PathBuf {
  argvus_config_home().join("hypr").join("hyprpaper.conf")
}

/// Returns the user-owned marker for a wallpaper selected independently of
/// the active theme.
fn custom_wallpaper_state_path() -> PathBuf {
  argvus_config_home().join(".wallpaper-custom")
}

/// Persists the selected wallpaper for the session service and future logins.
fn persist_custom_wallpaper(wallpaper: &Path) -> Result<(), String> {
  let path = custom_wallpaper_state_path();
  write_atomic(&path, &format!("{}\n", wallpaper.display()))?;
  persist_canonical_config_value(
    "/appearance/wallpaper",
    Value::String(wallpaper.to_string_lossy().into_owned()),
  )
}

/// Executes the `active_wallpaper` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn active_wallpaper() -> Option<String> {
  let content = fs::read_to_string(hyprpaper_config_path()).ok()?;
  let path = content.lines().find_map(|line| {
    let (key, value) = line.split_once('=')?;
    if key.trim() != "path" {
      return None;
    }
    let value = value.trim();
    let expanded = value.strip_prefix('~').map(|rest| {
      let home = argvus_control_center_core::paths::home();
      home.join(rest).to_string_lossy().to_string()
    });
    Some(expanded.unwrap_or_else(|| value.to_string()))
  })?;
  path
    .strip_prefix(WALLPAPERS_DIR)
    .map(|name| name.trim_start_matches('/').to_string())
}

/// Executes the `first_monitor` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn first_monitor() -> Option<String> {
  let output = SystemProcessRunner
    .run(&ProcessRequest::new("hyprctl").arg("monitors"))
    .ok()?;
  if output.status.is_none_or(|status| status != 0) {
    return None;
  }
  String::from_utf8_lossy(&output.stdout)
    .lines()
    .find_map(|line| line.strip_prefix("Monitor "))?
    .split_whitespace()
    .next()
    .map(str::to_string)
}

/// Writes `monitor =` and `path =` lines into the user hyprpaper.conf,
/// keeping the existing block form used by `hypr-wallpaper-pick.sh`.
fn write_hyprpaper_config(wallpaper: &Path) -> Result<(), String> {
  let config_path = hyprpaper_config_path();
  let existing = fs::read_to_string(&config_path).unwrap_or_default();
  let relative = wallpaper.to_string_lossy().replace(
    &argvus_control_center_core::paths::home()
      .to_string_lossy()
      .to_string(),
    "~",
  );
  let monitor = first_monitor().unwrap_or_default();
  if existing
    .lines()
    .any(|line| line.trim_start().starts_with("path ="))
  {
    let mut text = String::new();
    for line in existing.lines() {
      let trimmed = line.trim_start();
      if trimmed.starts_with("path =") {
        text.push_str(&format!("  path = {relative}\n"));
      } else {
        text.push_str(line);
        text.push('\n');
      }
    }
    if !text.contains("monitor =") {
      text = format!("  monitor = {monitor}\n{text}");
    } else {
      let mut patched = String::new();
      for line in text.lines() {
        if line.trim_start().starts_with("monitor =") {
          patched.push_str(&format!("  monitor = {monitor}\n"));
        } else {
          patched.push_str(line);
          patched.push('\n');
        }
      }
      text = patched;
    }
    write_atomic(&config_path, &text)?;
    return Ok(());
  }
  let text = format!(
    "wallpaper {{\n  monitor = {monitor}\n  path = {relative}\n  fit_mode = cover\n}}\n\npreload = {relative}\n"
  );
  write_atomic(&config_path, &text)
}

/// Applies the `write_atomic` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
  if let Some(parent) = path.parent()
    && !parent.exists()
  {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }
  let tmp = path.with_extension("tmp");
  fs::write(&tmp, content).map_err(|error| error.to_string())?;
  fs::rename(&tmp, path).map_err(|error| error.to_string())
}

/// Retrieves data for `load_state` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
/// Collect only data used by the requested page. Existing values survive unrelated refreshes.
pub fn load_page(
  page: crate::model::AppearancePage,
  mut state: AppearanceState,
) -> AppearanceState {
  use crate::model::AppearancePage;
  let canonical_config = canonical_config_document();
  let theme = canonical_config
    .as_ref()
    .and_then(|document| canonical_config_string(document, "/appearance/theme"))
    .map(|theme| canonical_theme_id(&theme))
    .unwrap_or_else(|| canonical_theme_id(&read_first(&active_theme_file(), DEFAULT_THEME)));
  state.theme = theme.clone();
  let default_accent = theme_default_accent(&theme).unwrap_or(DEFAULT_ACCENT);
  let legacy_accent = read_first(&accent_file(), default_accent);
  let persisted_accent = canonical_config
    .as_ref()
    .and_then(|document| canonical_config_string(document, "/appearance/accent"));
  let accent_source = persisted_accent.as_deref().unwrap_or(&legacy_accent);
  state.accent = normalize_hex_color(accent_source).unwrap_or_else(|| default_accent.to_string());
  if page == AppearancePage::Wallpapers {
    state.wallpapers = list_wallpapers();
    state.wallpaper_active = active_wallpaper();
  }
  if matches!(
    page,
    AppearancePage::Effects
      | AppearancePage::Taskbar
      | AppearancePage::WidgetTelemetry
      | AppearancePage::ControlPanel
      | AppearancePage::SurfaceSection { .. }
  ) {
    state.animations = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/animations"))
      .unwrap_or_else(|| effect_state("animations"));
    state.transparency = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/transparency"))
      .unwrap_or_else(|| effect_state("transparency"));
    state.blur = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/blur"))
      .unwrap_or_else(|| effect_state("blur"));
  }
  if matches!(
    page,
    AppearancePage::Effects
      | AppearancePage::BlurIntensity
      | AppearancePage::Taskbar
      | AppearancePage::WidgetTelemetry
      | AppearancePage::ControlPanel
      | AppearancePage::ControlCenter
      | AppearancePage::SurfaceSection { .. }
  ) {
    state.taskbar_transparency = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_integer(document, "/effects/transparency_taskbar_value")
      })
      .unwrap_or_else(|| effect_value("transparency", EffectSurface::Taskbar));
    state.control_panel_transparency = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_integer(document, "/effects/transparency_control-panel_value")
      })
      .unwrap_or_else(|| effect_value("transparency", EffectSurface::ControlPanel));
    state.widget_telemetry_transparency = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_integer(document, "/effects/transparency_widget-telemetry_value")
      })
      .unwrap_or_else(|| effect_value("transparency", EffectSurface::WidgetTelemetry));
    state.taskbar_blur = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_integer(document, "/effects/blur_global_value"))
      .unwrap_or_else(|| effect_value("blur", EffectSurface::Taskbar));
    state.blur_global_value = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_integer(document, "/effects/blur_global_value"))
      .unwrap_or_else(|| effect_value("blur", EffectSurface::Taskbar));
    state.control_panel_blur = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_integer(document, "/effects/blur_global_value"))
      .unwrap_or_else(|| effect_value("blur", EffectSurface::ControlPanel));
    state.widget_telemetry_blur = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_integer(document, "/effects/blur_global_value"))
      .unwrap_or_else(|| effect_value("blur", EffectSurface::WidgetTelemetry));
    state.terminal_transparency = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_integer(document, "/effects/transparency_terminal_value")
      })
      .unwrap_or_else(|| effect_value("transparency", EffectSurface::Terminal));
    state.terminal_blur = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_integer(document, "/effects/blur_global_value"))
      .unwrap_or_else(|| effect_value("blur", EffectSurface::Terminal));
    state.control_center_transparency = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_integer(document, "/effects/transparency_control-center_value")
      })
      .unwrap_or_else(|| effect_value("transparency", EffectSurface::ControlCenter));
    state.taskbar_transparency_enabled = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/transparency_taskbar_enabled"))
      .unwrap_or_else(|| effect_enabled("transparency", EffectSurface::Taskbar));
    state.control_panel_transparency_enabled = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_bool(document, "/effects/transparency_control-panel_enabled")
      })
      .unwrap_or_else(|| effect_enabled("transparency", EffectSurface::ControlPanel));
    state.widget_telemetry_transparency_enabled = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_bool(document, "/effects/transparency_widget-telemetry_enabled")
      })
      .unwrap_or_else(|| effect_enabled("transparency", EffectSurface::WidgetTelemetry));
    state.taskbar_blur_enabled = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/blur_taskbar_enabled"))
      .unwrap_or_else(|| effect_enabled("blur", EffectSurface::Taskbar));
    state.control_panel_blur_enabled = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/blur_control-panel_enabled"))
      .unwrap_or_else(|| effect_enabled("blur", EffectSurface::ControlPanel));
    state.widget_telemetry_blur_enabled = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_bool(document, "/effects/blur_widget-telemetry_enabled")
      })
      .unwrap_or_else(|| effect_enabled("blur", EffectSurface::WidgetTelemetry));
    state.terminal_transparency_enabled = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_bool(document, "/effects/transparency_terminal_enabled")
      })
      .unwrap_or_else(|| effect_enabled("transparency", EffectSurface::Terminal));
    state.terminal_blur_enabled = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/blur_terminal_enabled"))
      .unwrap_or_else(|| effect_enabled("blur", EffectSurface::Terminal));
    state.control_center_transparency_enabled = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_bool(document, "/effects/transparency_control-center_enabled")
      })
      .unwrap_or_else(|| effect_enabled("transparency", EffectSurface::ControlCenter));
    state.control_center_blur_enabled = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/effects/blur_control-center_enabled"))
      .unwrap_or_else(|| effect_enabled("blur", EffectSurface::ControlCenter));
  }
  if page == AppearancePage::WidgetTelemetry
    || matches!(
      page,
      AppearancePage::SurfaceSection {
        surface: EffectSurface::WidgetTelemetry,
        ..
      }
    )
  {
    state.widget_telemetry = canonical_config
      .as_ref()
      .and_then(|document| {
        canonical_config_bool(document, "/control_panel/widget_telemetry_enabled")
      })
      .unwrap_or_else(telemetry_state);
    state.widget_telemetry_blocks = canonical_config
      .as_ref()
      .map(canonical_widget_telemetry_blocks)
      .unwrap_or_else(telemetry_blocks);
  }
  if page == AppearancePage::ControlPanel
    || matches!(
      page,
      AppearancePage::SurfaceSection {
        surface: EffectSurface::ControlPanel,
        ..
      }
    )
  {
    let runtime_cards = control_panel_cards();
    state.control_panel_cards = canonical_config
      .as_ref()
      .map(|document| canonical_config_cards(document, runtime_cards.clone()))
      .unwrap_or(runtime_cards);
    state.control_panel_enabled = canonical_config
      .as_ref()
      .and_then(|document| canonical_config_bool(document, "/control_panel/enabled"))
      .unwrap_or_else(control_panel_enabled);
  }
  if matches!(
    page,
    AppearancePage::Themes
      | AppearancePage::ThemeFamilies { .. }
      | AppearancePage::CustomThemes
      | AppearancePage::ThemeModes { .. }
  ) {
    state.custom_themes = custom_themes();
    state.active_custom_theme = active_custom_theme();
  }
  if matches!(
    page,
    AppearancePage::SpacesBordersPosition
      | AppearancePage::Taskbar
      | AppearancePage::TaskbarPosition
      | AppearancePage::TaskbarSpaces
      | AppearancePage::WindowSpaces
      | AppearancePage::GeneralBorders
      | AppearancePage::EdgeThickness
      | AppearancePage::SurfaceSection {
        surface: EffectSurface::Taskbar,
        ..
      }
  ) {
    if let Some(output) = run_script_output(&script("taskbar-right-2-mode.sh"), &["status"]) {
      state.taskbar_utility_group = TaskbarUtilityGroupMode::from_value(output.trim());
    }
    if let Some(output) = run_script_output(&script("spaces-switch.sh"), &["--status"]) {
      parse_spacing_status(&output, &mut state);
    }
    if let Some(output) = run_script_output(&script("borders-switch.sh"), &["--status"]) {
      parse_borders_status(&output, &mut state);
    }
    if let Some(document) = canonical_config.as_ref() {
      if let Some(position) = canonical_config_string(document, "/layout/waybar_pos") {
        state.waybar_pos = TaskbarPosition::from_value(&position);
      }
      for (pointer, target) in [
        ("/layout/waybar_top", &mut state.waybar_top),
        ("/layout/waybar_left", &mut state.waybar_left),
        ("/layout/waybar_right", &mut state.waybar_right),
        ("/layout/waybar_bottom", &mut state.waybar_bottom),
        ("/layout/gaps_in", &mut state.gaps_in),
        ("/layout/gaps_out_top", &mut state.gaps_out_top),
        ("/layout/gaps_out_left", &mut state.gaps_out_left),
        ("/layout/gaps_out_right", &mut state.gaps_out_right),
        ("/layout/gaps_out_bottom", &mut state.gaps_out_bottom),
        ("/layout/rounding", &mut state.rounding),
        ("/layout/thickness", &mut state.thickness),
      ] {
        if let Some(value) = canonical_config_integer(document, pointer) {
          *target = value;
        }
      }
      if let Some(value) = canonical_config_bool(document, "/layout/rounded") {
        state.rounded = value;
      }
    }
  }
  state
}

/// Applies the `set_theme` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_theme(name: &str) -> Result<(), String> {
  let canonical_name = canonical_theme_id(name);
  persist_canonical_config_value("/appearance/theme", Value::String(canonical_name.clone()))?;
  run_script(&script("theme-switch.sh"), &[canonical_name.as_str()])?;
  let _ = fs::remove_file(argvus_config_home().join("state").join("custom-theme"));
  Ok(())
}

pub(crate) fn set_theme_static(name: &str) -> Result<(), String> {
  let canonical_name = canonical_theme_id(name);
  persist_canonical_config_value("/appearance/theme", Value::String(canonical_name.clone()))?;
  run_script_with_env(
    &script("theme-switch.sh"),
    &[canonical_name.as_str()],
    &[("ARGVUS_NO_RUNTIME", "1"), ("ARGVUS_THEME_SWITCH", "1")],
  )?;
  let _ = fs::remove_file(argvus_config_home().join("state").join("custom-theme"));
  Ok(())
}

/// Applies the `set_accent` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_accent(color: &str) -> Result<(), String> {
  let normalized_color =
    normalize_hex_color(color).ok_or_else(|| "invalid accent color".to_string())?;
  persist_canonical_config_value("/appearance/accent", Value::String(normalized_color))?;
  run_script(&script("accent-switch.sh"), &[color])
}

/// Applies the `set_animations` operation through the shared session helper.
pub fn set_animations(enabled: bool) -> Result<(), String> {
  persist_canonical_config_value("/effects/animations", Value::Bool(enabled))?;
  run_script(
    &script("effects-toggle.sh"),
    &["animations", if enabled { "enable" } else { "disable" }],
  )
}

pub fn set_effect_value(kind: &str, surface: EffectSurface, value: i32) -> Result<(), String> {
  if !matches!(kind, "transparency" | "blur") || !(0..=100).contains(&value) {
    return Err("invalid effect value".into());
  }
  let value = value.to_string();
  run_script(
    &script("effects-toggle.sh"),
    &[&format!("{kind}-value"), surface.key(), "set", &value],
  )
}

pub fn set_effect_component(component: &str, enabled: bool) -> Result<(), String> {
  if !matches!(component, "animations" | "transparency" | "blur") {
    return Err("invalid effect component".into());
  }
  run_script(
    &script("effects-toggle.sh"),
    &[component, if enabled { "enable" } else { "disable" }],
  )
}

pub fn apply_surface_effects(
  surface: EffectSurface,
  transparency_enabled: bool,
  transparency: i32,
  blur_enabled: bool,
  blur: i32,
) -> Result<(), String> {
  if !(0..=100).contains(&transparency) || !(0..=100).contains(&blur) {
    return Err("invalid effect value".into());
  }
  let args = [
    "surface-apply".to_string(),
    surface.key().to_string(),
    if transparency_enabled {
      "enabled"
    } else {
      "disabled"
    }
    .into(),
    transparency.to_string(),
    if blur_enabled { "enabled" } else { "disabled" }.into(),
    blur.to_string(),
  ];
  let references = args.iter().map(String::as_str).collect::<Vec<_>>();
  run_script(&script("effects-toggle.sh"), &references)
}

pub fn set_control_panel_enabled(enabled: bool) -> Result<(), String> {
  persist_canonical_config_value("/control_panel/enabled", Value::Bool(enabled))?;
  run_script(
    &control_panel_cards_script(),
    &[
      "master",
      "set",
      if enabled { "enabled" } else { "disabled" },
    ],
  )
}

pub fn apply_widget_telemetry(enabled: bool, blocks: &WidgetTelemetryBlocks) -> Result<(), String> {
  persist_canonical_config_value(
    "/control_panel/widget_telemetry_enabled",
    Value::Bool(enabled),
  )?;
  let enabled_blocks = WidgetTelemetryBlock::ALL
    .iter()
    .filter(|block| blocks.enabled(**block))
    .map(|block| Value::String(block.key().to_string()))
    .collect();
  persist_canonical_config_value(
    "/control_panel/widget_telemetry_blocks",
    Value::Array(enabled_blocks),
  )?;
  let values = WidgetTelemetryBlock::ALL.map(|block| {
    if blocks.enabled(block) {
      "enabled"
    } else {
      "disabled"
    }
  });
  let mut args = vec!["apply-state", if enabled { "enabled" } else { "disabled" }];
  args.extend(values);
  let mut request = ProcessRequest::new("argvus-widget-telemetry-toggle");
  for argument in args {
    request = request.arg(argument);
  }
  let output = SystemProcessRunner
    .run(&request)
    .map_err(|error| error.to_string())?;
  if output.status.is_none_or(|status| status != 0) {
    return Err(terminal_text(&String::from_utf8_lossy(&output.stderr)).to_string());
  }
  Ok(())
}

/// Applies the `set_wallpaper` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_wallpaper(name: &str) -> Result<(), String> {
  if name.is_empty()
    || name
      .chars()
      .any(|character| character.is_control() || character == '\n')
  {
    return Err("invalid wallpaper name".into());
  }
  let relative = Path::new(name);
  if relative.is_absolute()
    || relative
      .components()
      .any(|component| matches!(component, std::path::Component::ParentDir))
  {
    return Err("invalid wallpaper path".into());
  }
  let path = PathBuf::from(WALLPAPERS_DIR).join(relative);
  if !path.is_file() || path.extension().is_none_or(|extension| extension != "jxl") {
    return Err(format!("papel de parede não encontrado: {name}"));
  }
  write_hyprpaper_config(&path)?;
  persist_custom_wallpaper(&path)?;
  let _ = SystemProcessRunner.run(
    &ProcessRequest::new("systemctl")
      .arg("--user")
      .arg("restart")
      .arg("argvus-wallpaper.service"),
  );
  if let Ok(output) = SystemProcessRunner.run(
    &ProcessRequest::new("sh")
      .arg(script("hyprlock-theme.sh").to_string_lossy().to_string())
      .arg("--invalidate"),
  ) {
    let _ = output;
  }
  Ok(())
}

/// Opens the configured File Manager in a window focused on top of the Control
/// Center. TUI managers are launched inside a dedicated terminal (the
/// `argvus-wallpaper-picker` class); GUI managers are raised via their WM class
/// or matched by PID to guarantee they appear in front.
pub fn choose_wallpaper() -> Result<(), String> {
  let file_manager =
    default_app("file_manager").ok_or_else(|| "file manager padrão não encontrado".to_string())?;
  let home = argvus_control_center_core::paths::home();

  if !is_tui_file_manager(&file_manager) {
    let binary = &file_manager[0];
    let class = gui_file_manager_class(binary);
    let child = Command::new(binary)
      .args(file_manager.iter().skip(1))
      .arg(home)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|error| format!("não foi possível abrir o File Manager padrão: {error}"))?;
    raise_file_manager_window(child.id(), class);
    return Ok(());
  }

  let selection = cache_home()
    .join("argvus")
    .join(format!("wallpaper-selection-{}", std::process::id()));
  if let Some(parent) = selection.parent() {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }
  let _ = fs::remove_file(&selection);

  let mut terminal_command = Command::new("argvus-tui-terminal");
  let mut terminal_args = vec![
    "--class".to_string(),
    "argvus-wallpaper-picker".to_string(),
    "--term".to_string(),
    "kitty".to_string(),
    "--profile".to_string(),
    "wallpaper-picker".to_string(),
    "--".to_string(),
  ];
  for argument in &file_manager {
    terminal_args.push(argument.clone());
  }
  terminal_args.push(format!("--chooser-file={}", selection.display()));
  terminal_args.push(home.to_string_lossy().to_string());

  let mut child = terminal_command
    .args(terminal_args)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .map_err(|error| format!("não foi possível abrir o File Manager padrão: {error}"))?;
  focus_wallpaper_picker(child.id());
  if child
    .wait()
    .map_err(|error| format!("File Manager padrão falhou: {error}"))?
    .code()
    .is_some_and(|status| status != 0)
  {
    return Err("File Manager padrão falhou".into());
  }

  let selected =
    fs::read_to_string(&selection).map_err(|_| "nenhuma imagem foi selecionada".to_string())?;
  let selected = selected.trim();
  if selected.is_empty() {
    return Err("nenhuma imagem foi selecionada".into());
  }
  let selected = Path::new(selected);
  if !selected.is_file() {
    return Err("o arquivo selecionado não existe".into());
  }
  write_hyprpaper_config(selected)?;
  persist_custom_wallpaper(selected)?;
  let _ = SystemProcessRunner.run(
    &ProcessRequest::new("systemctl")
      .arg("--user")
      .arg("restart")
      .arg("argvus-wallpaper.service"),
  );
  let _ = fs::remove_file(selection);
  Ok(())
}

/// Applies the `set_spacing` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_spacing(key: &str, value: &str) -> Result<(), String> {
  if key.is_empty() || value.is_empty() {
    return Err("invalid spaces key/value".into());
  }
  persist_canonical_config_value(&format!("/layout/{key}"), Value::String(value.to_string()))?;
  run_script(&script("spaces-switch.sh"), &["--set-persist", key, value])?;
  run_script(&script("spaces-switch.sh"), &["--apply"])
}

/// Applies the `set_waybar_position` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_waybar_position(position: &str) -> Result<(), String> {
  if position != "top" && position != "bottom" {
    return Err("invalid waybar position".into());
  }
  persist_canonical_config_value("/layout/waybar_pos", Value::String(position.to_string()))?;
  run_script(
    &script("spaces-switch.sh"),
    &["--set-persist", "waybar_pos", position],
  )?;
  run_script(&script("spaces-switch.sh"), &["--apply"])
}

/// Persists the taskbar utility-group mode, regenerates its managed Waybar
/// configuration, and restarts the session components that consume it.
pub fn set_taskbar_utility_group(mode: TaskbarUtilityGroupMode) -> Result<(), String> {
  persist_canonical_config_value(
    "/layout/taskbar_utility_group",
    Value::String(mode.value().to_string()),
  )?;
  run_script(&script("taskbar-right-2-mode.sh"), &["set", mode.value()])?;
  reload_session()
}

/// Persists a complete batch of Control Panel preferences and performs one
/// targeted restart after all writes have succeeded.
pub fn set_control_panel_cards(changes: Vec<(ControlPanelCard, bool)>) -> Result<(), String> {
  let runtime_cards = control_panel_cards();
  let mut effective_cards = canonical_config_document()
    .as_ref()
    .map(|document| canonical_config_cards(document, runtime_cards.clone()))
    .unwrap_or(runtime_cards);
  for &(card, enabled) in &changes {
    effective_cards.set(card, enabled);
  }
  persist_canonical_config_value(
    "/control_panel/cards",
    control_panel_cards_value(&effective_cards),
  )?;
  for &(card, enabled) in &changes {
    run_script(
      &control_panel_cards_script(),
      &[
        "set",
        card.key(),
        if enabled { "enabled" } else { "disabled" },
      ],
    )?;
  }
  if changes.is_empty() {
    return Ok(());
  }
  reload_control_panel()
}

/// Restarts only the Control Panel consumer instead of reapplying every
/// mutable session component. This keeps rapid card toggles responsive while
/// preserving the required immediate panel reload.
fn reload_control_panel() -> Result<(), String> {
  let output = SystemProcessRunner
    .run(
      &ProcessRequest::new("argvus-sessionctl")
        .arg("restart")
        .arg("control-panel"),
    )
    .map_err(|error| error.to_string())?;
  if output.status.is_none_or(|status| status != 0) {
    let stderr = terminal_text(&String::from_utf8_lossy(&output.stderr))
      .trim()
      .to_string();
    return Err(if stderr.is_empty() {
      "argvus-sessionctl restart control-panel falhou".into()
    } else {
      stderr
    });
  }
  Ok(())
}

fn reload_session() -> Result<(), String> {
  let output = SystemProcessRunner
    .run(&ProcessRequest::new("argvus-sessionctl").arg("reload"))
    .map_err(|error| error.to_string())?;
  if output.status.is_none_or(|status| status != 0) {
    let stderr = terminal_text(&String::from_utf8_lossy(&output.stderr))
      .trim()
      .to_string();
    return Err(if stderr.is_empty() {
      "argvus-sessionctl reload falhou".into()
    } else {
      stderr
    });
  }
  Ok(())
}

pub(crate) fn reload_session_for_profile() -> Result<(), String> {
  reload_session()
}

/// Applies the `set_border` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_border(key: &str, value: &str) -> Result<(), String> {
  if key.is_empty() || value.is_empty() {
    return Err("invalid border key/value".into());
  }
  persist_canonical_config_value(&format!("/layout/{key}"), Value::String(value.to_string()))?;
  run_script(&script("borders-switch.sh"), &["--set-persist", key, value])?;
  run_script(&script("borders-switch.sh"), &["--apply"])?;

  // `borders-switch.sh --apply` updates the current Hyprland process and
  // generated styles. Session reload is still required for the managed
  // components to consume the persisted border contract, and the Control
  // Center is not one of the components restarted by argvus-sessionctl.
  reload_session()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wallpaper_paths_are_grouped_by_collection_and_mode() {
    assert_eq!(
      classify_wallpaper(Path::new("argvus-dark.jxl")),
      Some((WallpaperCollection::Abstract, WallpaperMode::Dark))
    );
    assert_eq!(
      classify_wallpaper(Path::new("argvus-light.jxl")),
      Some((WallpaperCollection::Abstract, WallpaperMode::Light))
    );
    assert_eq!(
      classify_wallpaper(Path::new("abstract/dark/gruvbox-abstract-dark.jxl")),
      Some((WallpaperCollection::Abstract, WallpaperMode::Dark))
    );
    assert_eq!(
      classify_wallpaper(Path::new("abstract/light/everforest-abstract-light.jxl")),
      Some((WallpaperCollection::Abstract, WallpaperMode::Light))
    );
    assert_eq!(
      classify_wallpaper(Path::new("landscape/light/gruvbox-landscape-light.jxl")),
      Some((WallpaperCollection::Landscape, WallpaperMode::Light))
    );
    assert_eq!(classify_wallpaper(Path::new("dark/wallpaper.jxl")), None);
    assert_eq!(
      classify_wallpaper(Path::new("abstract/dark/readme.png")),
      None
    );
  }

  #[test]
  /// Executes the `theme_default_accents_exist` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn theme_default_accents_exist() {
    assert_eq!(theme_default_accent("slate-dark-float"), Some("#7391a5"));
    assert_eq!(theme_default_accent("unknown"), None);
  }

  #[test]
  /// Executes the `status_parsers_ignore_unknown_keys` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn status_parsers_ignore_unknown_keys() {
    let mut state = AppearanceState::default();
    parse_spacing_status(
      "waybar_pos=top\nwaybar_top=16\nwaybar_left=14\nwaybar_right=14\nwaybar_bottom=0\ngaps_in=8\ngaps_out_top=8\ngaps_out_left=10\ngaps_out_right=10\ngaps_out_bottom=8\nfuture_property=123\n",
      &mut state,
    );
    parse_borders_status(
      "rounded=1\nrounding=4\nthickness=1\nfuture_property=123\n",
      &mut state,
    );
    assert_eq!(state.waybar_top, 16);
    assert_eq!(state.gaps_out_left, 10);
    assert!(state.rounded);
    assert_eq!(state.rounding, 4);
  }

  #[test]
  fn control_panel_status_keeps_defaults_and_applies_known_cards() {
    let cards = parse_control_panel_cards(
      r#"{"cards":[
        {"id":"weather","enabled":false},
        {"id":"future-card","enabled":false},
        {"id":"power","enabled":true}
      ]}"#,
    );
    assert!(!cards.enabled(ControlPanelCard::Weather));
    assert!(cards.enabled(ControlPanelCard::Power));
    assert!(cards.enabled(ControlPanelCard::User));
    assert!(parse_control_panel_cards("invalid").enabled(ControlPanelCard::User));
  }

  #[test]
  fn canonical_configuration_overrides_appearance_values() {
    let document = serde_json::json!({
      "appearance": {"theme": "dracula", "accent": "#BD93F9"},
      "layout": {"gaps_in": 12},
      "control_panel": {"cards": [{"id": "power", "enabled": false}]}
    });
    assert_eq!(
      canonical_config_string(&document, "/appearance/theme").as_deref(),
      Some("dracula")
    );
    assert_eq!(
      canonical_config_integer(&document, "/layout/gaps_in"),
      Some(12)
    );
    let cards = canonical_config_cards(&document, ControlPanelCards::default());
    assert!(!cards.enabled(ControlPanelCard::Power));
    assert!(cards.enabled(ControlPanelCard::User));
  }

  #[test]
  fn canonical_configuration_accepts_legacy_string_numbers() {
    let document = serde_json::json!({"layout": {"gaps_in": "9"}});
    assert_eq!(
      canonical_config_integer(&document, "/layout/gaps_in"),
      Some(9)
    );
  }

  #[test]
  /// Executes the `gui_file_manager_class_known_apps` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn gui_file_manager_class_known_apps() {
    assert_eq!(
      gui_file_manager_class("nautilus"),
      Some("org.gnome.Nautilus")
    );
    assert_eq!(gui_file_manager_class("thunar"), Some("Thunar"));
    assert_eq!(gui_file_manager_class("nemo"), Some("org.nemo.Nemo"));
    assert_eq!(gui_file_manager_class("dolphin"), Some("org.kde.dolphin"));
    assert_eq!(gui_file_manager_class("pcmanfm"), Some("pcmanfm"));
    assert_eq!(
      gui_file_manager_class("/usr/bin/nautilus"),
      Some("org.gnome.Nautilus")
    );
    assert_eq!(gui_file_manager_class("unknown-fm"), None);
    assert_eq!(gui_file_manager_class("spf"), None);
  }
}
