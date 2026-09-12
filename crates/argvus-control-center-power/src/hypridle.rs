use argvus_control_center_core::paths::argvus_config_home;
use std::fs;
use std::path::PathBuf;

/// The ARGVUS hypridle configuration lives next to `hyprland.lua`, under the
/// standard generated-config strategy. Only the DPMS listener timeout is
/// touched; everything else in the user config is preserved byte-for-byte.
pub fn config_path() -> Option<PathBuf> {
  let path = argvus_config_home().join("hypr").join("hypridle.conf");
  path.exists().then_some(path)
}

/// Returns the screen-off idle timeout in whole minutes.
pub fn screen_off_minutes() -> Option<u32> {
  let path = config_path()?;
  let content = fs::read_to_string(path).ok()?;
  screen_off_minutes_from(&content)
}

/// Updates the timeout that feeds the DPMS-off listener in `content` and
/// returns the new full configuration text. Returns an error when the config
/// has no recognizable DPMS listener, so we never guess at foreign layouts.
pub fn set_screen_off_minutes(content: &str, minutes: u32) -> Result<String, String> {
  let target = find_dpms_timeout(content).ok_or_else(|| {
    "O hypridle do ARGVUS não define um listener de desligar tela; nada foi alterado.".to_string()
  })?;
  let new_seconds = minutes.saturating_mul(60);
  let mut updated = String::with_capacity(content.len());
  for (index, line) in content.lines().enumerate() {
    if index == target {
      let indent: String = line
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect();
      updated.push_str(&format!("{indent}timeout = {new_seconds}\n"));
    } else {
      updated.push_str(line);
      updated.push('\n');
    }
  }
  Ok(updated)
}

/// Writes a new DPMS timeout back to the ARGVUS hypridle config. Returns the
/// previous timeout when the change was applied.
pub fn apply_screen_off_minutes(minutes: u32) -> Result<u32, String> {
  let Some(path) = config_path() else {
    return Err("arquivo hypridle.conf do ARGVUS não encontrado".into());
  };
  if fs::symlink_metadata(&path)
    .map(|metadata| metadata.file_type().is_symlink())
    .unwrap_or(false)
  {
    return Err("recusando alterar um hypridle.conf simbólico".into());
  }
  let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
  let previous = screen_off_minutes_from(&content).unwrap_or_default();
  let updated = set_screen_off_minutes(&content, minutes)?;
  let tmp = path.with_extension("tmp");
  fs::write(&tmp, updated).map_err(|error| error.to_string())?;
  fs::rename(&tmp, &path).map_err(|error| error.to_string())?;
  Ok(previous)
}

fn screen_off_minutes_from(content: &str) -> Option<u32> {
  let target = find_dpms_timeout(content)?;
  content
    .lines()
    .enumerate()
    .filter(|(index, line)| {
      *index == target && line.trim_start().starts_with("timeout") && line.contains('=')
    })
    .map(|(_, line)| {
      line
        .split_once('=')
        .and_then(|(_, value)| value.trim().parse::<u32>().ok())
    })
    .next()
    .flatten()
    .map(|seconds| (seconds / 60).max(1))
}

/// Locates the `timeout =` line that precedes a `hyprctl ... dpms off`
/// listener, which is hypridle's canonical screen-off idiom.
fn find_dpms_timeout(content: &str) -> Option<usize> {
  let mut current_timeout: Option<usize> = None;
  let mut target: Option<usize> = None;
  for (index, line) in content.lines().enumerate() {
    let trimmed = line.trim();
    if trimmed.starts_with('#') || trimmed.is_empty() {
      continue;
    }
    if trimmed.starts_with("timeout") && trimmed.contains('=') {
      current_timeout = Some(index);
    } else if trimmed.starts_with("on-timeout") && trimmed.contains('=') {
      let value = trimmed
        .split_once('=')
        .map(|(_, value)| value)
        .unwrap_or("");
      let value = value.trim();
      if calls_dpms_off(value) {
        target = current_timeout;
      }
      current_timeout = None;
    }
  }
  target
}

fn calls_dpms_off(value: &str) -> bool {
  let compact: String = value
    .chars()
    .filter(|character| !character.is_whitespace())
    .collect::<String>()
    .to_ascii_lowercase();
  compact.contains("dpmsoff") || compact.contains("dpms_state") || compact.contains("dpms,off")
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = "\
# ARGVUS idle
timeout = 300
onetime = false
label = dpms
on-timeout = hyprctl dispatch dpms off
on-resume = hyprctl dispatch dpms on

timeout = 1800
onetime = false
label = suspend
on-timeout = systemctl suspend
";

  #[test]
  fn finds_the_dpms_listener_timeout() {
    assert_eq!(screen_off_minutes_from(SAMPLE), Some(5));
  }

  #[test]
  fn rewrites_only_the_dpms_timeout() {
    let updated = set_screen_off_minutes(SAMPLE, 30).unwrap();
    assert!(updated.contains("# ARGVUS idle"), "{updated}");
    assert!(updated.contains("timeout = 1800\n"), "{updated}");
    assert!(updated.contains("systemctl suspend"), "{updated}");
    assert!(!updated.contains("timeout = 300\n"), "{updated}");
    assert_eq!(screen_off_minutes_from(&updated), Some(30));
  }

  #[test]
  fn recognizes_an_off_idiom_without_an_installer() {
    assert!(calls_dpms_off("hyprctl dispatch dpms off"));
    assert!(calls_dpms_off("hyprctl dispatch dpms_state 0"));
    assert!(!calls_dpms_off("systemctl suspend"));
    assert!(!calls_dpms_off("hyprctl dispatch dpms on"));
  }

  #[test]
  fn refuses_configs_without_a_dpms_listener() {
    let idle_only = "\
timeout = 1800
onetime = false
on-timeout = systemctl suspend
";
    assert!(set_screen_off_minutes(idle_only, 10).is_err());
    assert!(screen_off_minutes_from(idle_only).is_none());
  }
}
