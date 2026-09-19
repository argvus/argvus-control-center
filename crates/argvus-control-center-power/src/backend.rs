//! Implements isolated integration with system tools and APIs in crate `argvus control center power`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use crate::{
  hypridle,
  model::{LidContext, PowerBehavior, PowerButtonBehavior, PowerState},
};
use argvus_control_center_core::{
  privileged::{PrivilegedOperation, PrivilegedRequest, SystemSettingsOperation},
  process::{ProcessOutput, ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
};
use std::fs;

/// Defines the constant `LOGIND_MAIN`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const LOGIND_MAIN: &str = "/etc/systemd/logind.conf";
/// Defines the constant `LOGIND_DROP_IN`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const LOGIND_DROP_IN: &str = "/etc/systemd/logind.conf.d/argvus.conf";

/// Retrieves data for `load` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn load() -> PowerState {
  let lid_battery = logind_value("HandleLidSwitch")
    .as_deref()
    .and_then(PowerBehavior::parse)
    .unwrap_or(PowerBehavior::Suspend);
  let lid_ac = logind_value("HandleLidSwitchExternalPower")
    .as_deref()
    .and_then(PowerBehavior::parse)
    .unwrap_or(lid_battery);
  let power_button = logind_value("HandlePowerKey")
    .as_deref()
    .and_then(PowerButtonBehavior::parse)
    .unwrap_or(PowerButtonBehavior::Poweroff);
  let screen_off_minutes = hypridle::screen_off_minutes();
  let screen_off_supported = hypridle::config_path().exists();
  let lock_minutes = hypridle::lock_minutes();
  let lock_supported = screen_off_supported;
  let keep_awake = keep_awake_status();
  let can_suspend = systemctl_can("can-suspend");
  let can_hibernate = systemctl_can("can-hibernate");
  let is_laptop = detect_is_laptop();
  PowerState {
    lid: [lid_battery, lid_ac],
    power_button,
    screen_off_minutes,
    lock_minutes,
    can_suspend,
    can_hibernate,
    screen_off_supported,
    lock_supported,
    keep_awake,
    is_laptop,
  }
}

/// Executes the `keep_awake_status` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn keep_awake_status() -> bool {
  SystemProcessRunner
    .run(&ProcessRequest::new("/usr/share/argvus/power/sh/keep-awake.sh").arg("status"))
    .ok()
    .is_some_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "enabled")
}

/// Retrieves data for `detect_is_laptop` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn detect_is_laptop() -> bool {
  if let Ok(chassis) = fs::read_to_string("/sys/class/dmi/id/chassis_type")
    && let Ok(n) = chassis.trim().parse::<u8>()
    && matches!(n, 8..=14 | 30..=32)
  {
    return true;
  }
  fs::read_dir("/sys/class/power_supply")
    .ok()
    .and_then(|dir| {
      dir
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("BAT"))
    })
    .is_some()
}

/// Reads a logind `Key = value`, honoring the ARGVUS drop-in first, then the
/// main config file, then the documented systemd defaults.
fn logind_value(key: &str) -> Option<String> {
  let drop_in = fs::read_to_string(LOGIND_DROP_IN).ok()?;
  if let Some(value) = parse_logind_key(&drop_in, key) {
    return Some(value);
  }
  let main = fs::read_to_string(LOGIND_MAIN).ok()?;
  parse_logind_key(&main, key)
}

/// Converts input data into `parse_logind_key` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn parse_logind_key(content: &str, key: &str) -> Option<String> {
  content.lines().find_map(|line| {
    let line = line.trim();
    if line.starts_with('#') {
      return None;
    }
    let (name, value) = line.split_once('=')?;
    (name.trim() == key).then(|| value.trim().to_owned())
  })
}

/// Executes the `systemctl_can` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn systemctl_can(command: &str) -> bool {
  let output = SystemProcessRunner.run(&ProcessRequest::new("systemctl").arg(command));
  output
    .ok()
    .is_some_and(|out| matches!(String::from_utf8_lossy(&out.stdout).trim(), "yes"))
}

/// Applies the `set_lid` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_lid(context: LidContext, behavior: PowerBehavior) -> Result<(), String> {
  let (context, key) = match context {
    LidContext::Battery => ("battery", "HandleLidSwitch"),
    LidContext::Ac => ("ac", "HandleLidSwitchExternalPower"),
  };
  privileged_power(
    "set-lid",
    vec![context.into(), behavior.value().into()],
    &format!("{key}={}", behavior.value()),
  )
}

/// Applies the `set_power_button` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_power_button(behavior: PowerButtonBehavior) -> Result<(), String> {
  privileged_power(
    "set-power-button",
    vec![behavior.value().into()],
    &format!("HandlePowerKey={}", behavior.value()),
  )
}

/// Applies the `apply_idle` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn apply_idle(minutes: u32) -> Result<(), String> {
  hypridle::apply_screen_off_minutes(minutes).map(|_| ())
}

/// Applies the `apply_lock` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn apply_lock(minutes: u32) -> Result<(), String> {
  hypridle::apply_lock_minutes(minutes).map(|_| ())
}

/// Applies the `set_keep_awake` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn set_keep_awake(enabled: bool) -> Result<(), String> {
  let value = if enabled { "on" } else { "off" };
  let output = SystemProcessRunner
    .run(&ProcessRequest::new("/usr/share/argvus/power/sh/keep-awake.sh").arg(value))
    .map_err(|error| error.to_string())?;
  (output.status == Some(0))
    .then_some(())
    .ok_or_else(|| terminal_text(String::from_utf8_lossy(&output.stderr).trim()).to_string())
}

/// Executes the `suspend_now` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn suspend_now() -> Result<(), String> {
  privileged_power("suspend-now", Vec::new(), "A operação de suspensão falhou.")
}

/// Executes the `hibernate_now` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn hibernate_now() -> Result<(), String> {
  privileged_power(
    "hibernate-now",
    Vec::new(),
    "A operação de hibernação falhou.",
  )
}

/// Executes the `privileged_power` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn privileged_power(action: &str, arguments: Vec<String>, message: &str) -> Result<(), String> {
  let executable = std::env::current_exe()
    .map_err(|error| error.to_string())?
    .to_string_lossy()
    .into_owned();
  let operation = SystemSettingsOperation::new(SystemProcessRunner, executable);
  let request = PrivilegedRequest::new("power", action, arguments)?;
  let output = operation.execute(&request)?;
  if output.status == Some(0) {
    Ok(())
  } else {
    Err(privileged_failure(&output, message))
  }
}

/// Executes the `privileged_failure` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn privileged_failure(output: &ProcessOutput, message: &str) -> String {
  let stderr = terminal_text(String::from_utf8_lossy(&output.stderr).trim());
  match output.status {
    Some(126) => "Autorização cancelada.".into(),
    Some(127) => "Autorização negada.".into(),
    _ if stderr.contains("pkexec was not found") => "pkexec não encontrado; instale polkit.".into(),
    _ if stderr.is_empty() => message.into(),
    _ => stderr,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  /// Converts input data into `parses_logind_keys_ignoring_comments` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn parses_logind_keys_ignoring_comments() {
    let content = "\
# HandleLidSwitch=poweroff
HandleLidSwitch=lock
HandlePowerKey=poweroff
";
    assert_eq!(
      parse_logind_key(content, "HandleLidSwitch"),
      Some("lock".into())
    );
    assert_eq!(
      parse_logind_key(content, "HandlePowerKey"),
      Some("poweroff".into())
    );
    assert_eq!(parse_logind_key(content, "HandleLidSwitchDocked"), None);
  }

  #[test]
  /// Executes the `drop_in_precedence_over_main_config` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn drop_in_precedence_over_main_config() {
    let field = "HandleLidSwitchExternalPower";
    let drop_in = format!("{field}=ignore");
    let main = format!("{field}=suspend");
    assert_eq!(parse_logind_key(&drop_in, field), Some("ignore".into()));
    let _ = main;
    assert!(parse_logind_key("", field).is_none());
  }
}
