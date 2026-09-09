use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{host, keyboard, locale, time};

const ELEVATED_ENV: &str = "ARGVUS_SYSTEM_SETTINGS_ELEVATED";
const LOCALE_GEN: &str = "/etc/locale.gen";

pub fn run(args: &[String]) -> Result<(), String> {
  validate_args(args)?;
  ensure_root(args)?;
  match args {
    [area, action, value] if area == "timezone" && action == "set" => {
      run_command("timedatectl", &["set-timezone", value])
    }
    [area, action, value] if area == "hostname" && action == "set" => {
      host::validate_hostname(value)?;
      run_command("hostnamectl", &["set-hostname", value])
    }
    [area, action, value] if area == "datetime" && action == "ntp" => {
      run_command("timedatectl", &["set-ntp", value])
    }
    [area, action, value] if area == "locale" && action == "set-lang" => {
      run_command("localectl", &["set-locale", &format!("LANG={value}")])
    }
    [area, action, values @ ..] if area == "locale" && action == "set-generated" => {
      set_generated_locales(values)
    }
    [area, action, layout] if area == "keyboard" && action == "layout" => {
      run_command("localectl", &["set-x11-keymap", layout])
    }
    [area, action, layout, variant] if area == "keyboard" && action == "variant" => {
      run_command("localectl", &["set-x11-keymap", layout, "", variant])
    }
    [area, action, keymap] if area == "keyboard" && action == "console-keymap" => {
      run_command("localectl", &["set-keymap", keymap])
    }
    _ => Err("unsupported system-settings command".to_string()),
  }
}

fn validate_args(args: &[String]) -> Result<(), String> {
  match args {
    [area, action, value] if area == "timezone" && action == "set" => {
      if time::is_valid_timezone_name(value) {
        Ok(())
      } else {
        Err(format!("invalid time zone: {value}"))
      }
    }
    [area, action, value] if area == "hostname" && action == "set" => {
      host::validate_hostname(value)
    }
    [area, action, value] if area == "datetime" && action == "ntp" => match value.as_str() {
      "true" | "false" => Ok(()),
      _ => Err("NTP value must be true or false".to_string()),
    },
    [area, action, value] if area == "locale" && action == "set-lang" => {
      if locale::is_locale_name(value) {
        Ok(())
      } else {
        Err(format!("invalid locale: {value}"))
      }
    }
    [area, action, values @ ..] if area == "locale" && action == "set-generated" => {
      for value in values {
        let Some((name, encoding)) = value.split_once(' ') else {
          return Err(format!("invalid locale.gen entry: {value}"));
        };
        if !locale::is_locale_name(name) || !locale::is_encoding_name(encoding) {
          return Err(format!("invalid locale.gen entry: {value}"));
        }
      }
      Ok(())
    }
    [area, action, layout] if area == "keyboard" && action == "layout" => {
      if keyboard::is_xkb_name(layout) {
        Ok(())
      } else {
        Err(format!("invalid keyboard layout: {layout}"))
      }
    }
    [area, action, layout, variant] if area == "keyboard" && action == "variant" => {
      if keyboard::is_xkb_name(layout) && (variant.is_empty() || keyboard::is_xkb_name(variant)) {
        Ok(())
      } else {
        Err("invalid keyboard variant".to_string())
      }
    }
    [area, action, keymap] if area == "keyboard" && action == "console-keymap" => {
      if keyboard::is_keymap_name(keymap) {
        Ok(())
      } else {
        Err(format!("invalid console keymap: {keymap}"))
      }
    }
    _ => {
      Err("usage: argvus-control-center system-settings <domain> <action> [value...]".to_string())
    }
  }
}

fn ensure_root(args: &[String]) -> Result<(), String> {
  if effective_uid() == 0 || std::env::var_os(ELEVATED_ENV).is_some() {
    return Ok(());
  }
  let pkexec = locate_pkexec().ok_or_else(|| "pkexec was not found; install polkit".to_string())?;
  let exe = std::env::current_exe().map_err(|error| error.to_string())?;
  let mut command = Command::new(pkexec);
  command
    .arg(exe)
    .arg("system-settings")
    .args(args)
    .env_clear()
    .env(ELEVATED_ENV, "1");
  if let Some(value) = std::env::var_os("DBUS_SESSION_BUS_ADDRESS") {
    command.env("DBUS_SESSION_BUS_ADDRESS", value);
  }
  if let Some(value) = std::env::var_os("XDG_RUNTIME_DIR") {
    command.env("XDG_RUNTIME_DIR", value);
  }
  let error = command.exec();
  Err(format!("failed to launch pkexec: {error}"))
}

fn effective_uid() -> u32 {
  fs::read_to_string("/proc/self/status")
    .ok()
    .and_then(|contents| {
      contents.lines().find_map(|line| {
        line
          .strip_prefix("Uid:")
          .and_then(|rest| rest.split_whitespace().nth(1))
          .and_then(|value| value.parse().ok())
      })
    })
    .unwrap_or(1)
}

fn locate_pkexec() -> Option<PathBuf> {
  std::env::var_os("PATH").and_then(|path| {
    std::env::split_paths(&path)
      .map(|dir| dir.join("pkexec"))
      .find(|candidate| candidate.is_file())
  })
}

fn set_generated_locales(values: &[String]) -> Result<(), String> {
  let path = Path::new(LOCALE_GEN);
  let contents = fs::read_to_string(path).map_err(|error| format!("{LOCALE_GEN}: {error}"))?;
  let entries = locale::parse_locale_gen(&contents);
  let valid: BTreeSet<String> = entries
    .iter()
    .map(|entry| format!("{} {}", entry.locale, entry.encoding))
    .collect();
  let selected: BTreeSet<String> = values.iter().cloned().collect();
  for value in &selected {
    if !valid.contains(value) {
      return Err(format!("locale is not listed in {LOCALE_GEN}: {value}"));
    }
  }
  let updated = locale::apply_locale_gen(&contents, &selected);
  if updated != contents {
    write_atomic(path, &updated)?;
  }
  run_command("locale-gen", &[])
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
  let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
  let tmp = path.with_extension("argvus-control-center.tmp");
  {
    let mut file = fs::File::create(&tmp).map_err(|error| error.to_string())?;
    file
      .set_permissions(fs::Permissions::from_mode(metadata.permissions().mode()))
      .map_err(|error| error.to_string())?;
    file
      .write_all(contents.as_bytes())
      .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
  }
  fs::rename(&tmp, path).map_err(|error| error.to_string())
}

fn run_command(command: &str, args: &[&str]) -> Result<(), String> {
  let output = Command::new(command)
    .args(args)
    .stdin(Stdio::null())
    .output()
    .map_err(|error| format!("{command}: {error}"))?;
  if output.status.success() {
    println!("ok");
    Ok(())
  } else {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
      format!("{command} exited with {}", output.status)
    } else {
      stderr
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rejects_injection_like_arguments() {
    assert!(validate_args(&["timezone".into(), "set".into(), "UTC;reboot".into()]).is_err());
    assert!(validate_args(&["hostname".into(), "set".into(), "bad name".into()]).is_err());
    assert!(validate_args(&["keyboard".into(), "layout".into(), "br".into()]).is_ok());
  }
}
