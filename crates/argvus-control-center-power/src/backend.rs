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

const LOGIND_MAIN: &str = "/etc/systemd/logind.conf";
const LOGIND_DROP_IN: &str = "/etc/systemd/logind.conf.d/argvus.conf";

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
  let can_suspend = systemctl_can("can-suspend");
  let can_hibernate = systemctl_can("can-hibernate");
  let is_laptop = detect_is_laptop();
  PowerState {
    lid: [lid_battery, lid_ac],
    power_button,
    screen_off_minutes,
    can_suspend,
    can_hibernate,
    screen_off_supported,
    is_laptop,
  }
}

fn detect_is_laptop() -> bool {
  if let Ok(chassis) = fs::read_to_string("/sys/class/dmi/id/chassis_type") {
    if let Ok(n) = chassis.trim().parse::<u8>() {
      if matches!(n, 8..=14 | 30..=32) {
        return true;
      }
    }
  }
  fs::read_dir("/sys/class/power_supply")
    .ok()
    .and_then(|dir| dir.flatten().find(|e| e.file_name().to_string_lossy().starts_with("BAT")))
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

fn systemctl_can(command: &str) -> bool {
  let output = SystemProcessRunner.run(&ProcessRequest::new("systemctl").arg(command));
  output
    .ok()
    .is_some_and(|out| matches!(String::from_utf8_lossy(&out.stdout).trim(), "yes"))
}

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

pub fn set_power_button(behavior: PowerButtonBehavior) -> Result<(), String> {
  privileged_power(
    "set-power-button",
    vec![behavior.value().into()],
    &format!("HandlePowerKey={}", behavior.value()),
  )
}

pub fn apply_idle(minutes: u32) -> Result<(), String> {
  hypridle::apply_screen_off_minutes(minutes).map(|_| ())
}

pub fn suspend_now() -> Result<(), String> {
  privileged_power("suspend-now", Vec::new(), "A operação de suspensão falhou.")
}

pub fn hibernate_now() -> Result<(), String> {
  privileged_power(
    "hibernate-now",
    Vec::new(),
    "A operação de hibernação falhou.",
  )
}

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
  fn drop_in_precedence_over_main_config() {
    let field = "HandleLidSwitchExternalPower";
    let drop_in = format!("{field}=ignore");
    let main = format!("{field}=suspend");
    assert_eq!(parse_logind_key(&drop_in, field), Some("ignore".into()));
    let _ = main;
    assert!(parse_logind_key("", field).is_none());
  }
}
