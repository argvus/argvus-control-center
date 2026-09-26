//! Implements input-device configuration in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use super::ratbag;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
/// Represents `InputSettings`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct InputSettings {
  pub mouse: MouseSettings,
  pub touchpad: TouchpadSettings,
  #[serde(skip)]
  pub devices: Devices,
  #[serde(skip)]
  pub ratbag: Vec<ratbag::Device>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
/// Represents `MouseSettings`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct MouseSettings {
  pub sensitivity: f64,
  pub accel_profile: String,
  pub natural_scroll: bool,
  pub scroll_factor: f64,
  pub left_handed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
/// Represents `TouchpadSettings`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct TouchpadSettings {
  pub natural_scroll: bool,
  pub tap_to_click: bool,
  pub tap_and_drag: bool,
  pub two_finger_right_click: bool,
  pub disable_while_typing: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
/// Represents `Devices`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Devices {
  pub mouse: bool,
  pub touchpad: bool,
}

impl Default for MouseSettings {
  /// Executes the `default` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default() -> Self {
    Self {
      sensitivity: 0.0,
      accel_profile: "adaptive".into(),
      natural_scroll: false,
      scroll_factor: 1.0,
      left_handed: false,
    }
  }
}
impl Default for TouchpadSettings {
  /// Executes the `default` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default() -> Self {
    Self {
      natural_scroll: true,
      tap_to_click: true,
      tap_and_drag: true,
      two_finger_right_click: true,
      disable_while_typing: true,
    }
  }
}
/// Executes the `path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn path() -> PathBuf {
  argvus_control_center_core::paths::argvus_config_home().join("input.toml")
}

/// Retrieves data for `load` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn load() -> InputSettings {
  let persisted = fs::read_to_string(path());
  let mut settings: InputSettings = canonical_settings()
    .or_else(|| {
      persisted
        .as_deref()
        .ok()
        .and_then(|v| toml::from_str(v).ok())
    })
    .unwrap_or_default();
  if persisted.is_err() {
    settings.load_runtime_values();
  }
  settings.devices = detect_devices();
  settings.ratbag = ratbag::devices();
  let _ = write_generated_hypr_input(&settings);
  settings
}

impl InputSettings {
  /// Retrieves data for `load_runtime_values` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn load_runtime_values(&mut self) {
    self.mouse.sensitivity = query_float("input:sensitivity").unwrap_or(self.mouse.sensitivity);
    self.mouse.scroll_factor =
      query_float("input:scroll_factor").unwrap_or(self.mouse.scroll_factor);
    self.mouse.natural_scroll =
      query_bool("input:natural_scroll").unwrap_or(self.mouse.natural_scroll);
    self.mouse.left_handed = query_bool("input:left_handed").unwrap_or(self.mouse.left_handed);
    self.mouse.accel_profile = query_string("input:accel_profile")
      .filter(|v| matches!(v.as_str(), "adaptive" | "flat"))
      .unwrap_or(self.mouse.accel_profile.clone());
    self.touchpad.natural_scroll =
      query_bool("input:touchpad:natural_scroll").unwrap_or(self.touchpad.natural_scroll);
    self.touchpad.tap_to_click =
      query_bool("input:touchpad:tap_to_click").unwrap_or(self.touchpad.tap_to_click);
    self.touchpad.tap_and_drag =
      query_bool("input:touchpad:tap_and_drag").unwrap_or(self.touchpad.tap_and_drag);
    self.touchpad.two_finger_right_click = query_bool("input:touchpad:clickfinger_behavior")
      .unwrap_or(self.touchpad.two_finger_right_click);
    self.touchpad.disable_while_typing = query_bool("input:touchpad:disable_while_typing")
      .unwrap_or(self.touchpad.disable_while_typing);
  }
}

/// Applies the `save` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn save(settings: &InputSettings) -> Result<(), String> {
  let target = path();
  fs::create_dir_all(target.parent().ok_or("invalid input configuration path")?)
    .map_err(|e| e.to_string())?;
  let tmp = target.with_extension("toml.tmp");
  fs::write(
    &tmp,
    toml::to_string_pretty(settings).map_err(|e| e.to_string())?,
  )
  .map_err(|e| e.to_string())?;
  write_canonical_settings(settings)?;
  fs::rename(tmp, target).map_err(|e| e.to_string())?;
  write_generated_hypr_input(settings)
}

fn canonical_settings() -> Option<InputSettings> {
  let output = Command::new("argvus-config")
    .args(["get", "/hyprland/input", "--raw"])
    .output()
    .ok()?;
  if !output.status.success() {
    return None;
  }
  serde_json::from_slice(&output.stdout).ok()
}

fn write_canonical_settings(settings: &InputSettings) -> Result<(), String> {
  let value = serde_json::to_string(settings).map_err(|error| error.to_string())?;
  match Command::new("argvus-config")
    .args(["set", "/hyprland/input", &value])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped())
    .status()
  {
    Ok(status) if status.success() => Ok(()),
    Ok(status) => Err(format!("argvus-config exited with {status}")),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(error) => Err(format!("argvus-config unavailable: {error}")),
  }
}

/// Executes the `generated_hypr_input_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn generated_hypr_input_path() -> PathBuf {
  argvus_control_center_core::paths::argvus_config_home().join("generated/hypr/input-settings.lua")
}

/// Executes the `lua_escape` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn lua_escape(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Applies the `write_generated_hypr_input` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn write_generated_hypr_input(settings: &InputSettings) -> Result<(), String> {
  let path = generated_hypr_input_path();
  fs::create_dir_all(path.parent().ok_or("invalid generated input path")?)
    .map_err(|e| e.to_string())?;
  let contents = format!(
    "-- Generated by argvus-control-center. Do not edit this file directly.\nreturn {{\n  sensitivity = {},\n  accel_profile = \"{}\",\n  natural_scroll = {},\n  scroll_factor = {},\n  left_handed = {},\n  touchpad = {{\n    natural_scroll = {},\n    tap_to_click = {},\n    tap_and_drag = {},\n    clickfinger_behavior = {},\n    disable_while_typing = {},\n  }},\n}}\n",
    settings.mouse.sensitivity,
    lua_escape(&settings.mouse.accel_profile),
    settings.mouse.natural_scroll,
    settings.mouse.scroll_factor,
    settings.mouse.left_handed,
    settings.touchpad.natural_scroll,
    settings.touchpad.tap_to_click,
    settings.touchpad.tap_and_drag,
    settings.touchpad.two_finger_right_click,
    settings.touchpad.disable_while_typing,
  );
  let tmp = path.with_extension("lua.tmp");
  let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
  file
    .write_all(contents.as_bytes())
    .map_err(|e| e.to_string())?;
  file.sync_all().map_err(|e| e.to_string())?;
  fs::rename(tmp, path).map_err(|e| e.to_string())
}

impl InputSettings {
  /// Applies the `toggle` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn toggle(&mut self, index: usize) -> Result<(), String> {
    let (key, value) = match index {
      3 => ("input:natural_scroll", !self.mouse.natural_scroll),
      5 => ("input:left_handed", !self.mouse.left_handed),
      7 => (
        "input:touchpad:natural_scroll",
        !self.touchpad.natural_scroll,
      ),
      8 => ("input:touchpad:tap_to_click", !self.touchpad.tap_to_click),
      9 => ("input:touchpad:tap_and_drag", !self.touchpad.tap_and_drag),
      10 => (
        "input:touchpad:clickfinger_behavior",
        !self.touchpad.two_finger_right_click,
      ),
      11 => (
        "input:touchpad:disable_while_typing",
        !self.touchpad.disable_while_typing,
      ),
      _ => return Ok(()),
    };
    self.set_bool(key, value)?;
    match index {
      3 => self.mouse.natural_scroll = value,
      5 => self.mouse.left_handed = value,
      7 => self.touchpad.natural_scroll = value,
      8 => self.touchpad.tap_to_click = value,
      9 => self.touchpad.tap_and_drag = value,
      10 => self.touchpad.two_finger_right_click = value,
      11 => self.touchpad.disable_while_typing = value,
      _ => {}
    }
    save(self)
  }

  /// Executes the `cycle` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn cycle(&mut self, index: usize, direction: i8) -> Result<(), String> {
    self.cycle_by(index, direction)
  }

  /// Apply an accumulated slider movement in one compositor/config write.
  /// This keeps keyboard auto-repeat and slider dragging from spawning one
  /// `hyprctl` process per intermediate step.
  pub fn cycle_by(&mut self, index: usize, direction: i8) -> Result<(), String> {
    match index {
      1 => {
        self.mouse.sensitivity =
          validate_sensitivity(self.mouse.sensitivity + f64::from(direction) * 0.1)?;
        apply("input:sensitivity", &self.mouse.sensitivity.to_string())?;
      }
      2 => {
        self.mouse.accel_profile = if self.mouse.accel_profile == "adaptive" {
          "flat"
        } else {
          "adaptive"
        }
        .into();
        apply("input:accel_profile", &self.mouse.accel_profile)?;
      }
      4 => {
        self.mouse.scroll_factor =
          validate_scroll_factor(self.mouse.scroll_factor + f64::from(direction) * 0.1)?;
        apply("input:scroll_factor", &self.mouse.scroll_factor.to_string())?;
      }
      _ => return Ok(()),
    }
    save(self)
  }

  /// Applies the `set_bool` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn set_bool(&self, key: &str, value: bool) -> Result<(), String> {
    apply(key, if value { "true" } else { "false" })
  }
}

/// Executes the `validate_sensitivity` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn validate_sensitivity(value: f64) -> Result<f64, String> {
  if (-1.0..=1.0).contains(&value) {
    Ok((value * 10.0).round() / 10.0)
  } else {
    Err("sensitivity must be between -1.0 and 1.0".into())
  }
}
/// Executes the `validate_scroll_factor` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn validate_scroll_factor(value: f64) -> Result<f64, String> {
  if (0.1..=10.0).contains(&value) {
    Ok((value * 10.0).round() / 10.0)
  } else {
    Err("scroll factor must be between 0.1 and 10.0".into())
  }
}
/// Executes the `speed_display` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn speed_display(value: f64) -> String {
  format!("{value:+.1}  [{}]", slider_bar(value, -1.0, 1.0))
}
/// Executes the `factor_display` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn factor_display(value: f64) -> String {
  format!("{value:.1}  [{}]", slider_bar(value, 0.1, 10.0))
}
/// Executes the `slider_bar` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn slider_bar(value: f64, minimum: f64, maximum: f64) -> String {
  /// Defines the constant `SLOTS`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
  const SLOTS: usize = 15;
  let position = (((value - minimum) / (maximum - minimum)) * (SLOTS - 1) as f64)
    .round()
    .clamp(0.0, (SLOTS - 1) as f64) as usize;
  (0..SLOTS)
    .map(|index| if index == position { '●' } else { '─' })
    .collect()
}
/// Executes the `accel_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn accel_label(value: &str) -> &str {
  match value {
    "flat" => "control_center.input_acceleration_flat",
    _ => "control_center.input_acceleration_adaptive",
  }
}

/// Applies the `apply` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn apply(key: &str, value: &str) -> Result<(), String> {
  let output = Command::new("hyprctl")
    .args(["keyword", key, value])
    .output()
    .map_err(|e| e.to_string())?;
  if output.status.success() {
    Ok(())
  } else {
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
  }
}

/// Retrieves data for `detect_devices` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn detect_devices() -> Devices {
  let output = Command::new("hyprctl")
    .args(["devices", "-j"])
    .output()
    .ok();
  let Some(output) = output else {
    return Devices::default();
  };
  let Ok(root) = serde_json::from_slice::<Value>(&output.stdout) else {
    return Devices::default();
  };
  devices_from_json(&root)
}

/// Executes the `devices_from_json` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn devices_from_json(root: &Value) -> Devices {
  let mice = root
    .get("mice")
    .and_then(Value::as_array)
    .is_some_and(|v| !v.is_empty());
  let touchpad = root
    .get("touch")
    .or_else(|| root.get("touchpads"))
    .and_then(Value::as_array)
    .is_some_and(|v| !v.is_empty());
  Devices {
    mouse: mice,
    touchpad,
  }
}

/// Executes the `query_value` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn query_value(key: &str) -> Option<Value> {
  let output = Command::new("hyprctl")
    .args(["getoption", key, "-j"])
    .output()
    .ok()?;
  serde_json::from_slice(&output.stdout).ok()
}
/// Executes the `query_float` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn query_float(key: &str) -> Option<f64> {
  query_value(key)?.get("float").and_then(Value::as_f64)
}
/// Executes the `query_bool` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn query_bool(key: &str) -> Option<bool> {
  query_value(key)?
    .get("int")
    .and_then(Value::as_i64)
    .map(|v| v != 0)
    .or_else(|| query_value(key)?.get("bool").and_then(Value::as_bool))
}
/// Executes the `query_string` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn query_string(key: &str) -> Option<String> {
  query_value(key)?
    .get("str")
    .and_then(Value::as_str)
    .map(str::to_string)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  /// Executes the `validates_ranges` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn validates_ranges() {
    assert!(validate_sensitivity(-1.0).is_ok());
    assert!(validate_sensitivity(1.1).is_err());
    assert!(validate_scroll_factor(0.1).is_ok());
    assert!(validate_scroll_factor(10.1).is_err());
  }
  #[test]
  /// Executes the `normalizes_profile` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn normalizes_profile() {
    assert_eq!(
      accel_label("flat"),
      "control_center.input_acceleration_flat"
    );
    assert_eq!(
      accel_label("custom"),
      "control_center.input_acceleration_adaptive"
    );
  }

  #[test]
  /// Retrieves data for `detects_current_hyprland_touch_device_key` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn detects_current_hyprland_touch_device_key() {
    let devices = devices_from_json(&serde_json::json!({
      "mice": [],
      "touch": [{"name": "test-touchpad"}]
    }));
    assert!(!devices.mouse);
    assert!(devices.touchpad);
  }

  #[test]
  /// Executes the `accepts_legacy_touchpads_device_key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn accepts_legacy_touchpads_device_key() {
    let devices = devices_from_json(&serde_json::json!({
      "mice": [{"name": "test-mouse"}],
      "touchpads": [{"name": "test-touchpad"}]
    }));
    assert!(devices.mouse);
    assert!(devices.touchpad);
  }

  #[test]
  /// Executes the `slider_display_tracks_normalized_value` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn slider_display_tracks_normalized_value() {
    assert!(speed_display(-1.0).contains("●──────────────"));
    assert!(speed_display(1.0).contains("──────────────●"));
    assert!(factor_display(0.1).contains("●──────────────"));
    assert!(factor_display(10.0).contains("──────────────●"));
  }
}
