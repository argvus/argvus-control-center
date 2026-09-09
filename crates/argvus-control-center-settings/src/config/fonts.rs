use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::paths;
use crate::error::SettingsError;
use crate::system::fonts::FontEntry;

const DEFAULT_FAMILY: &str = "IBM Plex Mono";
const MANAGED_START: &str = "/* argvus-control-center-fonts:start */";
const MANAGED_END: &str = "/* argvus-control-center-fonts:end */";
const LEGACY_MANAGED_START: &str = "/* argvus-settings-fonts:start */";
const LEGACY_MANAGED_END: &str = "/* argvus-settings-fonts:end */";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontTarget {
  Taskbar,
  Sysinfo,
  ControlPanel,
  System,
  Apps,
  Terminal,
  Browser,
}

impl FontTarget {
  pub const ALL: [Self; 7] = [
    Self::Taskbar,
    Self::Sysinfo,
    Self::ControlPanel,
    Self::System,
    Self::Apps,
    Self::Terminal,
    Self::Browser,
  ];

  pub const fn key(self) -> &'static str {
    match self {
      Self::Taskbar => "taskbar",
      Self::Sysinfo => "sysinfo",
      Self::ControlPanel => "control_panel",
      Self::System => "system",
      Self::Apps => "apps",
      Self::Terminal => "terminal",
      Self::Browser => "browser",
    }
  }

  pub const fn default_size(self) -> u16 {
    match self {
      Self::Taskbar | Self::System | Self::Terminal => 13,
      Self::Sysinfo | Self::ControlPanel => 14,
      Self::Apps => 12,
      Self::Browser => 10,
    }
  }

  const fn default_family(self) -> &'static str {
    DEFAULT_FAMILY
  }

  const fn default_style(self) -> &'static str {
    "Regular"
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
  Antialiasing,
  Hinting,
  Subpixel,
  Dpi,
}

impl SettingKind {
  pub const ALL: [Self; 4] = [Self::Antialiasing, Self::Hinting, Self::Subpixel, Self::Dpi];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSelection {
  pub family: String,
  pub style: String,
  pub size: u16,
}

impl FontSelection {
  pub fn display_name(&self) -> String {
    FontEntry {
      family: self.family.clone(),
      style: self.style.clone(),
    }
    .display_name()
  }

  fn value(&self) -> String {
    format!("{} {}", self.display_name(), self.size)
  }
}

#[derive(Debug, Clone)]
pub struct FontSettings {
  profile: HashMap<FontTarget, FontSelection>,
  pub antialias: bool,
  pub hinting: String,
  pub subpixel: String,
  pub custom_dpi: bool,
  pub dpi: u16,
}

impl FontSettings {
  pub fn load() -> Self {
    let state = parse_state(&fs::read_to_string(paths::fonts_file()).unwrap_or_default());
    let mut profile = HashMap::new();
    for target in FontTarget::ALL {
      let key = target.key();
      let family = state
        .get(&format!("{key}_family"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| DEFAULT_FAMILY.to_string());
      let style = state
        .get(&format!("{key}_style"))
        .cloned()
        .unwrap_or_else(|| "Regular".to_string());
      let size = state
        .get(&format!("{key}_size"))
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| target.default_size())
        .clamp(8, 32);
      profile.insert(
        target,
        FontSelection {
          family,
          style,
          size,
        },
      );
    }
    Self {
      profile,
      antialias: state.get("antialias").is_none_or(|value| value == "true"),
      hinting: state
        .get("hinting")
        .cloned()
        .unwrap_or_else(|| "full".to_string()),
      subpixel: state
        .get("subpixel")
        .cloned()
        .unwrap_or_else(|| "none".to_string()),
      custom_dpi: state.get("custom_dpi").is_some_and(|value| value == "true"),
      dpi: state
        .get("dpi")
        .and_then(|value| value.parse().ok())
        .unwrap_or(96)
        .clamp(72, 240),
    }
  }

  pub fn get(&self, target: FontTarget) -> &FontSelection {
    self
      .profile
      .get(&target)
      .expect("every font target is initialized")
  }

  pub fn apply_font(
    &mut self,
    target: FontTarget,
    font: &FontEntry,
    size: u16,
  ) -> Result<(), SettingsError> {
    let previous = self.get(target).clone();
    self.profile.insert(
      target,
      FontSelection {
        family: font.family.clone(),
        style: font.style.clone(),
        size: size.clamp(8, 32),
      },
    );
    let result = self.apply_target(target);
    if result.is_err() {
      self.profile.insert(target, previous);
      let _ = self.write_state();
    }
    result.map_err(SettingsError::Fonts)
  }

  pub fn reset_font(&mut self, target: FontTarget) -> Result<(), SettingsError> {
    let previous = self.get(target).clone();
    self.profile.insert(
      target,
      FontSelection {
        family: target.default_family().to_string(),
        style: target.default_style().to_string(),
        size: target.default_size(),
      },
    );
    let result = self.apply_target(target);
    if result.is_err() {
      self.profile.insert(target, previous);
      let _ = self.write_state();
    }
    result.map_err(SettingsError::Fonts)
  }

  pub fn reset_setting(&mut self, setting: SettingKind) -> Result<(), SettingsError> {
    let value = match setting {
      SettingKind::Antialiasing => "enabled",
      SettingKind::Hinting => "full",
      SettingKind::Subpixel => "none",
      SettingKind::Dpi => "automatic",
    };
    self.apply_setting(setting, value)
  }

  pub fn reset_all(&mut self) -> Result<(), SettingsError> {
    let previous = self.clone();
    *self = Self::defaults();
    let result = (|| {
      self.write_state()?;
      for target in FontTarget::ALL {
        self.apply_target(target)?;
      }
      self.apply_rendering()?;
      self.refresh_runtime();
      Ok(())
    })();
    if result.is_err() {
      *self = previous;
      let _ = self.write_state();
    }
    result.map_err(SettingsError::Fonts)
  }

  pub fn apply_setting(&mut self, setting: SettingKind, value: &str) -> Result<(), SettingsError> {
    let previous = self.clone();
    match setting {
      SettingKind::Antialiasing => self.antialias = value == "enabled",
      SettingKind::Hinting => self.hinting = value.to_string(),
      SettingKind::Subpixel => self.subpixel = value.to_string(),
      SettingKind::Dpi => {
        if value == "automatic" {
          self.custom_dpi = false;
        } else {
          self.dpi = value
            .parse::<u16>()
            .map_err(|error| SettingsError::Fonts(error.to_string()))?
            .clamp(72, 240);
          self.custom_dpi = true;
        }
      }
    }
    let result = (|| {
      self.write_state()?;
      if setting != SettingKind::Dpi {
        self.apply_rendering()?;
      }
      self.refresh_runtime();
      Ok(())
    })();
    if result.is_err() {
      *self = previous;
      let _ = self.write_state();
    }
    result.map_err(SettingsError::Fonts)
  }

  pub fn setting_value(&self, setting: SettingKind) -> String {
    match setting {
      SettingKind::Antialiasing => if self.antialias {
        "enabled"
      } else {
        "disabled"
      }
      .to_string(),
      SettingKind::Hinting => self.hinting.clone(),
      SettingKind::Subpixel => self.subpixel.clone(),
      SettingKind::Dpi => {
        if self.custom_dpi {
          self.dpi.to_string()
        } else {
          "automatic".to_string()
        }
      }
    }
  }

  fn apply_target(&self, target: FontTarget) -> Result<(), String> {
    self.write_state()?;
    match target {
      FontTarget::Taskbar | FontTarget::Sysinfo => self.write_waybar_settings()?,
      FontTarget::ControlPanel => {}
      FontTarget::System => gsettings_set(
        "org.gnome.desktop.wm.preferences",
        "titlebar-font",
        &self.get(target).value(),
      )?,
      FontTarget::Apps => {
        let value = self.get(target).value();
        gsettings_set("org.gnome.desktop.interface", "font-name", &value)?;
        gsettings_set("org.gnome.desktop.interface", "document-font-name", &value)?;
        self.write_gtk_settings()?;
        self.write_rofi_settings()?;
      }
      FontTarget::Terminal => {
        gsettings_set(
          "org.gnome.desktop.interface",
          "monospace-font-name",
          &self.get(target).value(),
        )?;
        self.write_terminal_settings()?;
      }
      FontTarget::Browser => self.write_browser_settings()?,
    }
    self.refresh_runtime();
    Ok(())
  }

  fn defaults() -> Self {
    let mut profile = HashMap::new();
    for target in FontTarget::ALL {
      profile.insert(
        target,
        FontSelection {
          family: target.default_family().to_string(),
          style: target.default_style().to_string(),
          size: target.default_size(),
        },
      );
    }
    Self {
      profile,
      antialias: true,
      hinting: "full".to_string(),
      subpixel: "none".to_string(),
      custom_dpi: false,
      dpi: 96,
    }
  }

  fn apply_rendering(&self) -> Result<(), String> {
    gsettings_set(
      "org.gnome.desktop.interface",
      "font-antialiasing",
      if self.antialias { "grayscale" } else { "none" },
    )?;
    gsettings_set("org.gnome.desktop.interface", "font-hinting", &self.hinting)?;
    gsettings_set(
      "org.gnome.desktop.interface",
      "font-rgba-order",
      &self.subpixel,
    )
  }

  fn write_state(&self) -> Result<(), String> {
    let mut output = String::from("# Written by argvus-control-center. Edit with care.\n");
    for target in FontTarget::ALL {
      let font = self.get(target);
      let key = target.key();
      let _ = writeln!(output, "{key}_family={}", font.family);
      let _ = writeln!(output, "{key}_style={}", font.style);
      let _ = writeln!(output, "{key}_size={}", font.size);
      let _ = writeln!(output, "{key}_name={}", font.display_name());
    }
    let apps = self.get(FontTarget::Apps);
    let terminal = self.get(FontTarget::Terminal);
    let _ = writeln!(output, "default_family={}", apps.family);
    let _ = writeln!(output, "default_style={}", apps.style);
    let _ = writeln!(output, "default_size={}", apps.size);
    let _ = writeln!(output, "default_name={}", apps.display_name());
    let _ = writeln!(output, "monospace_family={}", terminal.family);
    let _ = writeln!(output, "monospace_style={}", terminal.style);
    let _ = writeln!(output, "monospace_size={}", terminal.size);
    let _ = writeln!(output, "monospace_name={}", terminal.display_name());
    let _ = writeln!(output, "antialias={}", self.antialias);
    let _ = writeln!(output, "hinting={}", self.hinting);
    let _ = writeln!(output, "subpixel={}", self.subpixel);
    let _ = writeln!(output, "custom_dpi={}", self.custom_dpi);
    let _ = writeln!(output, "dpi={}", self.dpi);
    write_file(&paths::fonts_file(), &output)
  }

  fn write_gtk_settings(&self) -> Result<(), String> {
    let value = self.get(FontTarget::Apps).value();
    for directory in [
      paths::config_home().join("gtk-3.0"),
      paths::config_home().join("gtk-4.0"),
      paths::argvus_config_home().join("gtk-3.0"),
      paths::argvus_config_home().join("gtk-4.0"),
    ] {
      replace_ini_setting(&directory.join("settings.ini"), "gtk-font-name", &value)?;
    }
    Ok(())
  }

  fn write_rofi_settings(&self) -> Result<(), String> {
    let system = paths::system_config_root().join("rofi/theme.rasi");
    let user_theme = paths::argvus_config_home().join("rofi/theme.rasi");
    let theme = if user_theme.exists() {
      user_theme
    } else {
      system
    };
    let font = escape(&self.get(FontTarget::Apps).value());
    let generated = paths::argvus_config_home().join("generated/rofi/config.rasi");
    write_file(
      &generated,
      &format!(
        "@theme \"{}\"\n\nconfiguration {{\n    font: \"{font}\";\n    show-icons: false;\n    sort: true;\n    case-sensitive: false;\n}}\n",
        escape(&theme.display().to_string())
      ),
    )?;
    let user = paths::argvus_config_home().join("rofi/config.rasi");
    if user.exists() {
      write_managed_block(
        &user,
        &format!("configuration {{\n    font: \"{font}\";\n}}\n"),
      )?;
    }
    Ok(())
  }

  fn write_waybar_settings(&self) -> Result<(), String> {
    let taskbar = self.get(FontTarget::Taskbar);
    let widget_telemetry = self.get(FontTarget::Sysinfo);
    self.write_waybar_profile(
      "argvus-taskbar.css",
      &format!(
        "* {{\n  font-family: \"{}\", \"Font Awesome 7 Free\", monospace;\n  font-size: {}px;\n}}\n",
        escape(&taskbar.family), taskbar.size
      ),
    )?;
    self.write_waybar_profile(
      "argvus-widget-telemetry.css",
      &format!(
        "* {{\n  font-family: \"{}\", \"Symbols Nerd Font Mono\", monospace;\n  font-size: {}px;\n}}\n",
        escape(&widget_telemetry.family), widget_telemetry.size
      ),
    )
  }

  fn write_waybar_profile(&self, name: &str, block: &str) -> Result<(), String> {
    let user = paths::argvus_config_home().join("waybar").join(name);
    if user.exists() {
      return write_managed_block(&user, block);
    }
    let system = paths::system_config_root().join("waybar").join(name);
    let generated = paths::argvus_config_home()
      .join("generated/waybar")
      .join(name);
    write_file(
      &generated,
      &format!(
        "@import url(\"{}\");\n\n{MANAGED_START}\n{block}{MANAGED_END}\n",
        escape(&system.display().to_string())
      ),
    )
  }

  fn write_terminal_settings(&self) -> Result<(), String> {
    let foot = paths::argvus_config_home().join("foot/foot.ini");
    if !foot.exists() {
      let system = paths::system_config_root().join("foot/foot.ini");
      if system.exists() {
        write_file(
          &foot,
          &fs::read_to_string(system).map_err(|error| error.to_string())?,
        )?;
      }
    }
    if foot.exists() {
      let font = self.get(FontTarget::Terminal);
      replace_prefixed_setting(
        &foot,
        "font",
        &format!(
          "{}:size={}, Noto Color Emoji:size=12",
          font.family, font.size
        ),
      )?;
    }
    spawn_if_available("argvus-terminal", &["--apply"]);
    Ok(())
  }

  fn write_browser_settings(&self) -> Result<(), String> {
    let font = self.get(FontTarget::Browser);
    let mut matches = String::new();
    for process in [
      "firefox",
      "chromium",
      "google-chrome",
      "google-chrome-stable",
      "brave",
      "brave-browser",
    ] {
      let _ = writeln!(
        matches,
        "  <match target=\"pattern\">\n    <test name=\"prgname\" compare=\"eq\"><string>{process}</string></test>\n    <edit name=\"family\" mode=\"prepend\" binding=\"strong\"><string>{}</string></edit>\n    <edit name=\"size\" mode=\"assign\"><double>{}</double></edit>\n  </match>",
        xml_escape(&font.family),
        font.size
      );
    }
    let file = paths::config_home().join("fontconfig/conf.d/52-argvus-browser-font.conf");
    write_file(
      &file,
      &format!(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\">\n<fontconfig>\n{matches}</fontconfig>\n"
      ),
    )?;
    spawn_if_available("fc-cache", &["-f"]);
    Ok(())
  }

  fn refresh_runtime(&self) {
    spawn_if_available("hyprctl", &["reload"]);
    spawn_if_available("argvus-sessionctl", &["restart", "waybar", "shell"]);
    let script = paths::system_config_root().join("scripts/argvus/hyprlock-theme.sh");
    if script.is_file() {
      let script = script.to_string_lossy();
      spawn_if_available("sh", &[script.as_ref(), "--invalidate"]);
    }
  }
}

pub fn parse_state(contents: &str) -> HashMap<String, String> {
  contents
    .lines()
    .filter_map(|line| {
      let line = line.trim();
      if line.is_empty() || line.starts_with('#') {
        return None;
      }
      line
        .split_once('=')
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
    })
    .collect()
}

fn gsettings_set(schema: &str, key: &str, value: &str) -> Result<(), String> {
  let status = Command::new("gsettings")
    .args(["set", schema, key, value])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .status()
    .map_err(|error| format!("gsettings unavailable: {error}"))?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("gsettings exited with {status}"))
  }
}

fn spawn_if_available(command: &str, args: &[&str]) {
  let _ = Command::new(command)
    .args(args)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn();
}

fn replace_ini_setting(path: &Path, key: &str, value: &str) -> Result<(), String> {
  let mut lines: Vec<String> = if path.exists() {
    fs::read_to_string(path)
      .map_err(|error| error.to_string())?
      .lines()
      .map(str::to_string)
      .collect()
  } else {
    vec!["[Settings]".to_string()]
  };
  if !lines.iter().any(|line| line.trim() == "[Settings]") {
    lines.insert(0, "[Settings]".to_string());
  }
  replace_or_append(&mut lines, key, &format!("{key}={value}"));
  write_file(path, &(lines.join("\n") + "\n"))
}

fn replace_prefixed_setting(path: &Path, key: &str, value: &str) -> Result<(), String> {
  let mut lines: Vec<String> = fs::read_to_string(path)
    .map_err(|error| error.to_string())?
    .lines()
    .map(str::to_string)
    .collect();
  replace_or_append(&mut lines, key, &format!("{key}      {value}"));
  write_file(path, &(lines.join("\n") + "\n"))
}

fn replace_or_append(lines: &mut Vec<String>, key: &str, replacement: &str) {
  if let Some(line) = lines.iter_mut().find(|line| {
    let trimmed = line.trim_start();
    trimmed.starts_with(&format!("{key}=")) || trimmed.starts_with(&format!("{key} "))
  }) {
    *line = replacement.to_string();
  } else {
    lines.push(replacement.to_string());
  }
}

fn write_managed_block(path: &Path, block: &str) -> Result<(), String> {
  let existing = fs::read_to_string(path).unwrap_or_default();
  let managed = format!("{MANAGED_START}\n{block}{MANAGED_END}\n");
  let markers = if existing.contains(MANAGED_START) {
    Some((MANAGED_START, MANAGED_END))
  } else if existing.contains(LEGACY_MANAGED_START) {
    Some((LEGACY_MANAGED_START, LEGACY_MANAGED_END))
  } else {
    None
  };
  let contents = if let Some((start_marker, end_marker)) = markers {
    if let Some(start) = existing.find(start_marker) {
      let relative_end = existing[start..].find(end_marker);
      if let Some(relative_end) = relative_end {
        let end = start + relative_end + end_marker.len();
        format!("{}{}{}", &existing[..start], managed, &existing[end..])
      } else {
        format!("{}\n{managed}", existing.trim_end())
      }
    } else {
      format!("{}\n{managed}", existing.trim_end())
    }
  } else {
    format!("{}\n{managed}", existing.trim_end())
  };
  write_file(path, &contents)
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }
  fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}

fn escape(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn xml_escape(value: &str) -> String {
  value
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_state_and_ignores_comments() {
    let state = parse_state("# header\napps_family=Noto Sans\napps_size=12\n");
    assert_eq!(
      state.get("apps_family").map(String::as_str),
      Some("Noto Sans")
    );
    assert_eq!(state.len(), 2);
  }

  #[test]
  fn all_targets_have_valid_defaults() {
    let settings = FontSettings::load();
    for target in FontTarget::ALL {
      assert!(!settings.get(target).family.is_empty());
      assert!((8..=32).contains(&settings.get(target).size));
    }
  }

  #[test]
  fn xml_values_are_escaped() {
    assert_eq!(xml_escape("A&B <Mono>"), "A&amp;B &lt;Mono&gt;");
  }
}
