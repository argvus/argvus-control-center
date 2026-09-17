#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearancePage {
  Home,
  Themes,
  ThemeModes { family: usize },
  Wallpapers,
  Accents,
  WaybarPosition,
  Prompt { goal: PromptGoal },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptGoal {
  /// Window inner gap (`general:gaps_in`).
  GapsIn,
  /// Window outer gap (`general:gaps_out`).
  GapsOut,
  /// Taskbar margin in pixels (`waybar` key in `.spaces`).
  Waybar,
}

impl PromptGoal {
  pub fn key(self) -> &'static str {
    match self {
      Self::GapsIn => "gaps_in",
      Self::GapsOut => "gaps_out",
      Self::Waybar => "waybar",
    }
  }
}

/// Every theme distributed by ARGVUS, in display order.
pub const THEMES: &[(&str, &str)] = &[
  ("argvus-dark-aether", "Argvus Dark Aether"),
  ("argvus-dark-aether-float", "Argvus Dark Aether Float"),
  ("argvus-dark-silver", "Argvus Dark Silver"),
  ("argvus-dark-silver-float", "Argvus Dark Silver Float"),
  ("argvus-dark-slate", "Argvus Dark Slate"),
  ("argvus-dark-slate-float", "Argvus Dark Slate Float"),
  ("argvus-dark-universe", "Argvus Dark Universe"),
  ("argvus-dark-universe-float", "Argvus Dark Universe Float"),
  ("argvus-light-veil", "Argvus Light Veil"),
  ("argvus-light-veil-float", "Argvus Light Veil Float"),
];

/// Theme families shown before selecting the Sticky or Float mode.
pub const THEME_FAMILIES: &[(&str, &str)] = &[
  ("argvus-dark-aether", "ARGVUS Dark Aether"),
  ("argvus-dark-silver", "ARGVUS Dark Silver"),
  ("argvus-dark-slate", "ARGVUS Dark Slate"),
  ("argvus-dark-universe", "ARGVUS Dark Universe"),
  ("argvus-light-veil", "ARGVUS Light Veil"),
];

pub fn theme_label(name: &str) -> String {
  THEMES
    .iter()
    .find(|(code, _)| *code == name)
    .map(|(_, label)| (*label).to_string())
    .unwrap_or_else(|| name.to_string())
}

pub fn theme_family_label(name: &str) -> String {
  let family = name.strip_suffix("-float").unwrap_or(name);
  THEME_FAMILIES
    .iter()
    .find(|(code, _)| *code == family)
    .map(|(_, label)| (*label).to_string())
    .unwrap_or_else(|| theme_label(family))
}

/// The highlight colors offered by the control panel, in display order.
pub const ACCENTS: &[(&str, &str)] = &[
  ("Blue", "#3590bd"),
  ("Slate Blue", "#7391a5"),
  ("Brown", "#996548"),
  ("Green", "#17d174"),
  ("Magenta", "#cb17d1"),
  ("Red", "#d1174f"),
  ("Yellow", "#d1ce17"),
  ("Purple", "#9617d1"),
  ("Silver", "#595959"),
];

pub fn accent_label(color: &str) -> String {
  ACCENTS
    .iter()
    .find(|(_, value)| *value == color)
    .map(|(label, _)| (*label).to_string())
    .unwrap_or_else(|| color.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceState {
  pub theme: String,
  pub accent: String,
  pub wallpapers: Vec<String>,
  pub wallpaper_active: Option<String>,
  pub effects: bool,
  pub widget_telemetry: bool,
  pub gaps_in: i32,
  pub gaps_out: i32,
  pub waybar: i32,
  /// "top" or "bottom".
  pub waybar_pos: String,
}

impl Default for AppearanceState {
  fn default() -> Self {
    Self {
      theme: "argvus-dark-aether".into(),
      accent: "#3590bd".into(),
      wallpapers: Vec::new(),
      wallpaper_active: None,
      effects: true,
      widget_telemetry: true,
      gaps_in: 3,
      gaps_out: 1,
      waybar: 0,
      waybar_pos: "top".into(),
    }
  }
}

impl AppearanceState {
  /// Whether the current theme name looks like a "float" variant.
  pub fn is_float_theme(&self) -> bool {
    self.theme.ends_with("-float")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_theme_and_accent_labels_resolve() {
    assert_eq!(theme_label("argvus-dark-aether"), "Argvus Dark Aether");
    assert_eq!(
      theme_family_label("argvus-dark-aether-float"),
      "ARGVUS Dark Aether"
    );
    assert_eq!(accent_label("#3590bd"), "Blue");
    assert_eq!(theme_label("custom-theme"), "custom-theme");
    assert_eq!(accent_label("#123456"), "#123456");
  }

  #[test]
  fn prompt_goals_map_to_spaces_keys() {
    assert_eq!(PromptGoal::GapsIn.key(), "gaps_in");
    assert_eq!(PromptGoal::GapsOut.key(), "gaps_out");
    assert_eq!(PromptGoal::Waybar.key(), "waybar");
  }

  #[test]
  fn float_theme_detection() {
    assert!(
      AppearanceState {
        theme: "argvus-dark-silver-float".into(),
        ..Default::default()
      }
      .is_float_theme()
    );
    assert!(!AppearanceState::default().is_float_theme());
  }
}
