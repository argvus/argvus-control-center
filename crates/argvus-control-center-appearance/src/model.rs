#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearancePage {
  Home,
  Themes,
  ThemeModes { family: usize },
  Wallpapers,
  Accents,
  SpacesBordersPosition,
  TaskbarPosition,
  TaskbarSpaces,
  WindowSpaces,
  GeneralBorders,
  EdgeThickness,
  Prompt { goal: PromptGoal },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarPosition {
  Top,
  Bottom,
}
impl TaskbarPosition {
  pub fn value(self) -> &'static str {
    match self {
      Self::Top => "top",
      Self::Bottom => "bottom",
    }
  }
  pub fn from_value(value: &str) -> Self {
    if value.trim() == "bottom" {
      Self::Bottom
    } else {
      Self::Top
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptGoal {
  WaybarTop,
  WaybarLeft,
  WaybarRight,
  WaybarBottom,
  GapsIn,
  GapsOutTop,
  GapsOutLeft,
  GapsOutRight,
  GapsOutBottom,
  Rounding,
  Thickness,
}
impl PromptGoal {
  pub fn key(self) -> &'static str {
    match self {
      Self::WaybarTop => "waybar_top",
      Self::WaybarLeft => "waybar_left",
      Self::WaybarRight => "waybar_right",
      Self::WaybarBottom => "waybar_bottom",
      Self::GapsIn => "gaps_in",
      Self::GapsOutTop => "gaps_out_top",
      Self::GapsOutLeft => "gaps_out_left",
      Self::GapsOutRight => "gaps_out_right",
      Self::GapsOutBottom => "gaps_out_bottom",
      Self::Rounding => "rounding",
      Self::Thickness => "thickness",
    }
  }
  pub const fn range(self) -> (i32, i32) {
    match self {
      Self::Rounding => (2, 10),
      Self::Thickness => (0, 10),
      _ => (0, 100),
    }
  }
}

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
  pub waybar_pos: TaskbarPosition,
  pub waybar_top: i32,
  pub waybar_left: i32,
  pub waybar_right: i32,
  pub waybar_bottom: i32,
  pub gaps_in: i32,
  pub gaps_out_top: i32,
  pub gaps_out_left: i32,
  pub gaps_out_right: i32,
  pub gaps_out_bottom: i32,
  pub rounded: bool,
  pub rounding: i32,
  pub thickness: i32,
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
      waybar_pos: TaskbarPosition::Top,
      waybar_top: 0,
      waybar_left: 0,
      waybar_right: 0,
      waybar_bottom: 0,
      gaps_in: 3,
      gaps_out_top: 1,
      gaps_out_left: 1,
      gaps_out_right: 1,
      gaps_out_bottom: 1,
      rounded: false,
      rounding: 0,
      thickness: 1,
    }
  }
}
impl AppearanceState {
  pub fn is_float_theme(&self) -> bool {
    self.theme.ends_with("-float")
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn prompt_metadata_is_explicit() {
    assert_eq!(
      (PromptGoal::WaybarTop.key(), PromptGoal::WaybarTop.range()),
      ("waybar_top", (0, 100))
    );
    assert_eq!(
      (
        PromptGoal::GapsOutBottom.key(),
        PromptGoal::GapsOutBottom.range()
      ),
      ("gaps_out_bottom", (0, 100))
    );
    assert_eq!(
      (PromptGoal::Rounding.key(), PromptGoal::Rounding.range()),
      ("rounding", (2, 10))
    );
    assert_eq!(
      (PromptGoal::Thickness.key(), PromptGoal::Thickness.range()),
      ("thickness", (0, 10))
    );
  }
  #[test]
  fn position_is_type_safe() {
    assert_eq!(
      TaskbarPosition::from_value("bottom"),
      TaskbarPosition::Bottom
    );
    assert_eq!(TaskbarPosition::Top.value(), "top");
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
  }
}
