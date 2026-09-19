//! Implements resource loading in crate `argvus theme`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use std::fs;
use std::path::{Path, PathBuf};

/// Defines the constant `DEFAULT_THEME`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub const DEFAULT_THEME: &str = "argvus-dark-aether";

/// Represents `Loader`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Loader {
  resource_dir: PathBuf,
  active_file: PathBuf,
  cache_file: PathBuf,
}

impl Loader {
  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn new() -> Self {
    let explicit = std::env::var_os("ARGVUS_CONTROL_CENTER_RESOURCE_DIR").map(PathBuf::from);
    let installed = std::env::var_os("ARGVUS_SYSTEM_CONFIG")
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from("/usr/share/argvus"))
      .join("control-center/config");
    let development = development_resources_dir();
    let resource_dir = explicit.unwrap_or_else(|| {
      if installed.is_dir() {
        installed
      } else {
        development
      }
    });
    let cache_home = std::env::var_os("XDG_CACHE_HOME")
      .map(PathBuf::from)
      .unwrap_or_else(|| home().join(".cache"));
    Self {
      resource_dir,
      active_file: argvus_config_home().join(".active-theme"),
      cache_file: cache_home.join("argvus-control-center/theme.css"),
    }
  }

  /// Executes the `active_name` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn active_name(&self) -> String {
    fs::read_to_string(&self.active_file)
      .ok()
      .map(|value| value.trim().to_string())
      .filter(|value| !value.is_empty())
      .unwrap_or_else(|| DEFAULT_THEME.to_string())
  }

  /// Executes the `sources` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn sources(&self) -> Vec<PathBuf> {
    let active = self.active_name();
    vec![
      self.resource_dir.join("style.css"),
      self.resource_dir.join("theme.css"),
      self
        .resource_dir
        .join("themes")
        .join(format!("{active}.css")),
      self.cache_file.clone(),
    ]
  }
}

impl Default for Loader {
  /// Executes the `default` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default() -> Self {
    Self::new()
  }
}

/// Executes the `argvus_config_home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn argvus_config_home() -> PathBuf {
  std::env::var_os("ARGVUS_CONFIG_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| {
      std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
    })
    .join("argvus")
}

/// Executes the `home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn home() -> PathBuf {
  std::env::var_os("HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Executes the `development_resources_dir` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn development_resources_dir() -> PathBuf {
  let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../src/usr/share/argvus/control-center/config");
  if source.is_dir() {
    return source;
  }
  std::env::current_dir()
    .ok()
    .and_then(|dir| {
      [
        dir.join("src/usr/share/argvus/control-center/config"),
        dir.join("../../src/usr/share/argvus/control-center/config"),
        dir.join("resources"),
        dir.join("argvus-control-center/resources"),
        dir.join("../../resources"),
      ]
      .into_iter()
      .find(|candidate| candidate.is_dir())
    })
    .unwrap_or_else(|| PathBuf::from("resources"))
}

/// Retrieves data for `read_recursive` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn read_recursive(path: &Path, stack: &mut Vec<PathBuf>) -> Option<String> {
  let contents = fs::read_to_string(path).ok()?;
  let mut output = String::new();
  for line in contents.lines() {
    let trimmed = line.trim();
    let import = trimmed
      .strip_prefix("@import url(")
      .and_then(|value| value.strip_suffix(");").or_else(|| value.strip_suffix(')')))
      .map(|value| value.trim().trim_matches(['\'', '"']))
      .filter(|value| !value.is_empty())
      .map(|value| path.parent().unwrap_or(Path::new(".")).join(value));
    if let Some(import) = import {
      if !stack.contains(&import) {
        stack.push(import.clone());
        if let Some(imported) = read_recursive(&import, stack) {
          output.push_str(&imported);
        }
        stack.pop();
      }
    } else {
      output.push_str(line);
      output.push('\n');
    }
  }
  Some(output)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  /// Executes the `every_official_theme_exposes_the_shared_palette` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn every_official_theme_exposes_the_shared_palette() {
    let themes = development_resources_dir().join("themes");
    let entries = fs::read_dir(themes).expect("official theme directory");
    let mut count = 0;
    for entry in entries.flatten() {
      if entry.path().extension().and_then(|value| value.to_str()) != Some("css") {
        continue;
      }
      let css = read_recursive(&entry.path(), &mut Vec::new()).expect("read official theme");
      let definitions = crate::parser::extract_define_colors(&css);
      assert!(
        definitions.iter().any(|(name, _)| name == "argvus_bg"),
        "{} has no background",
        entry.path().display()
      );
      assert!(
        definitions.iter().any(|(name, _)| name == "argvus_accent"),
        "{} has no accent",
        entry.path().display()
      );
      count += 1;
    }
    assert!(count >= 10, "expected all packaged ARGVUS themes");
  }
}
