//! Read/write the single `defaults.json` state file consumed by Argvus.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::catalog::{Category, STATE_VERSION};

/// The `defaults.json` content. Kept as a plain struct so the file stays
/// simple and readable; absent categories simply fall back to Argvus defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppState {
  pub version: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub terminal: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file_manager: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub text_editor: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub terminal_editor: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub browser: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub image_viewer: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pdf_viewer: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub video_player: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub audio_player: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub archive: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub launcher: Option<String>,
}

impl AppState {
  pub fn new() -> Self {
    AppState {
      version: Some(STATE_VERSION),
      ..Default::default()
    }
  }

  fn field(&mut self, cat: Category) -> &mut Option<String> {
    macro_rules! field {
      ($c:expr, $name:ident) => {
        if cat == $c {
          return &mut self.$name;
        }
      };
    }
    field!(Category::Terminal, terminal);
    field!(Category::FileManager, file_manager);
    field!(Category::TextEditor, text_editor);
    field!(Category::TerminalEditor, terminal_editor);
    field!(Category::Browser, browser);
    field!(Category::ImageViewer, image_viewer);
    field!(Category::PdfViewer, pdf_viewer);
    field!(Category::VideoPlayer, video_player);
    field!(Category::AudioPlayer, audio_player);
    field!(Category::Archive, archive);
    field!(Category::Launcher, launcher);
    unreachable!("category {} not covered", cat.key())
  }

  fn field_ref(&self, cat: Category) -> Option<&String> {
    match cat {
      Category::Terminal => self.terminal.as_ref(),
      Category::FileManager => self.file_manager.as_ref(),
      Category::TextEditor => self.text_editor.as_ref(),
      Category::TerminalEditor => self.terminal_editor.as_ref(),
      Category::Browser => self.browser.as_ref(),
      Category::ImageViewer => self.image_viewer.as_ref(),
      Category::PdfViewer => self.pdf_viewer.as_ref(),
      Category::VideoPlayer => self.video_player.as_ref(),
      Category::AudioPlayer => self.audio_player.as_ref(),
      Category::Archive => self.archive.as_ref(),
      Category::Launcher => self.launcher.as_ref(),
    }
  }

  /// Stored value for a category (not the fallback).
  pub fn get(&self, cat: Category) -> Option<String> {
    self.field_ref(cat).cloned()
  }

  /// Effective value for a category: stored value, else the Argvus fallback.
  pub fn effective(&self, cat: Category) -> String {
    match self.get(cat) {
      Some(v) if !v.is_empty() => v,
      _ => cat.fallback().to_string(),
    }
  }

  /// Set a stored value (empty clears the entry).
  pub fn set(&mut self, cat: Category, value: impl Into<String>) {
    let value = value.into();
    if value.is_empty() {
      *self.field(cat) = None;
    } else {
      *self.field(cat) = Some(value);
    }
  }

  /// Categories that differ from the Argvus fallback (i.e. explicit picks).
  #[allow(dead_code)]
  pub fn explicit(&self) -> Vec<(Category, String)> {
    Category::ORDER
      .iter()
      .copied()
      .filter_map(|c| self.get(c).map(|v| (c, v)))
      .filter(|(c, v)| !v.is_empty() && *v != c.fallback())
      .collect()
  }

  /// Load the state file; a missing/invalid file yields an empty state.
  pub fn load() -> AppState {
    let primary = argvus_control_center_core::paths::defaults_file();
    if primary.exists() {
      return Self::load_from(&primary);
    }
    let legacy = argvus_control_center_core::paths::legacy_defaults_file();
    if legacy.exists() {
      return Self::load_from(&legacy);
    }
    AppState::new()
  }

  /// Load from a specific path (used by tests and by the GUI).
  pub fn load_from(path: &PathBuf) -> AppState {
    let content = match fs::read_to_string(path) {
      Ok(c) => c,
      Err(_) => return AppState::new(),
    };
    match serde_json::from_str::<AppState>(&content) {
      Ok(mut state) => {
        state.version.get_or_insert(STATE_VERSION);
        state
      }
      Err(_) => AppState::new(),
    }
  }

  /// Persist to the state file (creating parent directories).
  pub fn save(&self) -> std::io::Result<()> {
    let path = argvus_control_center_core::paths::defaults_file();
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(self).expect("AppState serializes to valid JSON");
    fs::write(&path, format!("{json}\n"))?;
    Ok(())
  }

  /// Persist to a specific path (tests).
  #[cfg(test)]
  pub fn save_to(&self, path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(self).unwrap();
    fs::write(path, format!("{json}\n"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::env;
  use std::fs;

  const DIR: &str = "target/test-state";

  fn tmp_file(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join(DIR)
      .join(name);
    if let Some(parent) = p.parent() {
      fs::create_dir_all(parent).unwrap();
    }
    p
  }

  #[test]
  fn round_trip() {
    let mut state = AppState::new();
    state.set(Category::Browser, "firefox");
    state.set(Category::Terminal, "kitty");

    let path = tmp_file("roundtrip.json");
    state.save_to(&path).unwrap();

    let loaded = AppState::load_from(&path);
    assert_eq!(loaded, state);
    assert_eq!(loaded.get(Category::Browser).as_deref(), Some("firefox"));
  }

  #[test]
  fn missing_file_yields_empty_state() {
    let path = tmp_file("missing.json");
    let _ = fs::remove_file(&path);
    let state = AppState::load_from(&path);
    assert_eq!(state.get(Category::Browser), None);
    assert_eq!(state.effective(Category::Terminal), "argvus-terminal");
  }

  #[test]
  fn invalid_json_yields_empty_state() {
    let path = tmp_file("invalid.json");
    fs::write(&path, "not json {").unwrap();
    let state = AppState::load_from(&path);
    assert_eq!(state.effective(Category::Terminal), "argvus-terminal");
  }

  #[test]
  fn set_clears_on_empty() {
    let mut state = AppState::new();
    state.set(Category::Browser, "firefox");
    assert_eq!(state.effective(Category::Browser), "firefox");
    state.set(Category::Browser, "");
    assert_eq!(state.effective(Category::Browser), "");
    assert_eq!(state.get(Category::Browser), None);
  }

  #[test]
  fn explicit_only_lists_differences() {
    let mut state = AppState::new();
    state.set(Category::Browser, "chromium");
    state.set(Category::Terminal, "argvus-terminal"); // equals fallback
    let explicit = state.explicit();
    assert_eq!(explicit.len(), 1);
    assert_eq!(explicit[0].0, Category::Browser);
  }

  #[test]
  fn env_config_home_is_respected() {
    let p = PathBuf::from("/tmp/argvus-default-apps-test-config");
    unsafe {
      env::set_var("ARGVUS_CONFIG_HOME", &p);
    }
    assert_eq!(
      argvus_control_center_core::paths::argvus_config_home(),
      p.join("argvus")
    );
  }
}
