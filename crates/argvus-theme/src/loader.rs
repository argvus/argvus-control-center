use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_THEME: &str = "argvus-dark-aether";

pub struct Loader {
  resource_dir: PathBuf,
  active_file: PathBuf,
  cache_file: PathBuf,
}

impl Loader {
  pub fn new() -> Self {
    let explicit = std::env::var_os("ARGVUS_CONTROL_CENTER_RESOURCE_DIR").map(PathBuf::from);
    let installed = PathBuf::from("/etc/argvus-control-center");
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
      cache_file: cache_home.join("argvus-calendar/theme.css"),
    }
  }

  pub fn active_name(&self) -> String {
    fs::read_to_string(&self.active_file)
      .ok()
      .map(|value| value.trim().to_string())
      .filter(|value| !value.is_empty())
      .unwrap_or_else(|| DEFAULT_THEME.to_string())
  }

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
  fn default() -> Self {
    Self::new()
  }
}

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

fn home() -> PathBuf {
  std::env::var_os("HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn development_resources_dir() -> PathBuf {
  std::env::current_dir()
    .ok()
    .and_then(|dir| {
      [
        dir.join("resources"),
        dir.join("argvus-control-center/resources"),
        dir.join("../../resources"),
      ]
      .into_iter()
      .find(|candidate| candidate.is_dir())
    })
    .unwrap_or_else(|| PathBuf::from("resources"))
}

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
