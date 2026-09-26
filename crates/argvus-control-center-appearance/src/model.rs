//! Implements domain state and models consumed by the UI in crate `argvus control center appearance`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.

use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

impl HexColor {
  pub fn normalized(self) -> String {
    format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
  }

  pub fn foreground(self) -> ratatui::style::Color {
    let luminance =
      (299 * u32::from(self.r) + 587 * u32::from(self.g) + 114 * u32::from(self.b)) / 1000;
    if luminance >= 128 {
      ratatui::style::Color::Black
    } else {
      ratatui::style::Color::White
    }
  }
}

impl FromStr for HexColor {
  type Err = String;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
      return Err("expected six hexadecimal digits".into());
    }
    Ok(Self {
      r: u8::from_str_radix(&value[0..2], 16).map_err(|_| "invalid red channel")?,
      g: u8::from_str_radix(&value[2..4], 16).map_err(|_| "invalid green channel")?,
      b: u8::from_str_radix(&value[4..6], 16).map_err(|_| "invalid blue channel")?,
    })
  }
}

pub fn normalize_hex_color(value: &str) -> Option<String> {
  value.parse::<HexColor>().ok().map(HexColor::normalized)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeCategory {
  Dark,
  Light,
}

impl ThemeCategory {
  pub const ALL: [Self; 2] = [Self::Dark, Self::Light];

  pub fn contains_family(self, family_id: &str) -> bool {
    match self {
      Self::Dark => matches!(
        family_id,
        "argvus-dark"
          | "argvus-dark-float"
          | "dracula"
          | "gruvbox-dark"
          | "gruvbox-high-dark"
          | "monokai-dark"
          | "one-dark"
          | "rose-pine"
          | "silver-dark"
          | "slate-dark"
          | "sunset"
          | "tokyo-night"
          | "hackerman"
          | "solitude"
          | "universe"
          | "dracula-float"
          | "gruvbox-dark-float"
          | "gruvbox-high-dark-float"
          | "monokai-dark-float"
          | "one-dark-float"
          | "rose-pine-float"
          | "silver-dark-float"
          | "slate-dark-float"
          | "sunset-float"
          | "tokyo-night-float"
          | "hackerman-float"
          | "solitude-float"
          | "universe-float"
      ),
      Self::Light => matches!(
        family_id,
        "argvus-light"
          | "argvus-light-float"
          | "catppuccin-latte"
          | "frost"
          | "github-light"
          | "gruvbox-light"
          | "solarized-light"
          | "one-light"
          | "everforest-light"
          | "catppuccin-latte-float"
          | "frost-float"
          | "github-light-float"
          | "gruvbox-light-float"
          | "solarized-light-float"
          | "one-light-float"
          | "everforest-light-float"
      ),
    }
  }

  pub fn label_key(self) -> &'static str {
    match self {
      Self::Dark => "control_center.theme_category_dark",
      Self::Light => "control_center.theme_category_light",
    }
  }

  pub fn for_family_id(family_id: &str) -> Option<Self> {
    Self::ALL
      .into_iter()
      .find(|category| category.contains_family(family_id))
  }
}

/// Canonicalizes identifiers from profiles and persisted state after the
/// official theme identifiers were simplified.
pub fn canonical_theme_id(theme_id: &str) -> String {
  match theme_id {
    "argvus-dark-aether" => "argvus-dark".into(),
    "argvus-dark-aether-float" => "argvus-dark-float".into(),
    "argvus-light-veil" => "argvus-light".into(),
    "argvus-light-veil-float" => "argvus-light-float".into(),
    "argvus-onedark" => "one-dark".into(),
    "argvus-onedark-float" => "one-dark-float".into(),
    "argvus-dark-dracula" => "dracula".into(),
    "argvus-dark-dracula-float" => "dracula-float".into(),
    "argvus-dark-silver" => "silver-dark".into(),
    "argvus-dark-silver-float" => "silver-dark-float".into(),
    "argvus-dark-slate" => "slate-dark".into(),
    "argvus-dark-slate-float" => "slate-dark-float".into(),
    "argvus-dark-universe" => "universe".into(),
    "argvus-dark-universe-float" => "universe-float".into(),
    "argvus-dark-gruvbox-high" => "gruvbox-high-dark".into(),
    "argvus-dark-gruvbox-high-float" => "gruvbox-high-dark-float".into(),
    "argvus-dark-gruvbox" => "gruvbox-dark".into(),
    "argvus-dark-gruvbox-float" => "gruvbox-dark-float".into(),
    "argvus-light-gruvbox" => "gruvbox-light".into(),
    "argvus-light-gruvbox-float" => "gruvbox-light-float".into(),
    "argvus-dark-rose-pine" | "argvus-dark-rosepine" => "rose-pine".into(),
    "argvus-dark-rose-pine-float" | "argvus-dark-rosepine-float" => "rose-pine-float".into(),
    "argvus-dark-tokio-night" | "argvus-dark-tokyo-night" => "tokyo-night".into(),
    "argvus-dark-tokio-night-float" | "argvus-dark-tokyo-night-float" => "tokyo-night-float".into(),
    "argvus-dark-solitude" => "solitude".into(),
    "argvus-dark-solitude-float" => "solitude-float".into(),
    "argvus-dark-sunset" => "sunset".into(),
    "argvus-dark-sunset-float" => "sunset-float".into(),
    "argvus-dark-hackerman" => "hackerman".into(),
    "argvus-dark-hackerman-float" => "hackerman-float".into(),
    "argvus-dark-monokai" => "monokai-dark".into(),
    "argvus-dark-monokai-float" => "monokai-dark-float".into(),
    "argvus-github-light" => "github-light".into(),
    "argvus-github-light-float" => "github-light-float".into(),
    "argvus-light-solarized" => "solarized-light".into(),
    "argvus-light-solarized-float" => "solarized-light-float".into(),
    "argvus-light-frost" => "frost".into(),
    "argvus-light-frost-float" => "frost-float".into(),
    "argvus-dark-catppuccin-latte"
    | "argvus-light-catppuccin-latte"
    | "argvus-catppuccin-latte" => "catppuccin-latte".into(),
    "argvus-dark-catppuccin-latte-float"
    | "argvus-light-catppuccin-latte-float"
    | "argvus-catppuccin-latte-float" => "catppuccin-latte-float".into(),
    _ => theme_id.to_owned(),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperCollection {
  Abstract,
  Landscape,
}

impl WallpaperCollection {
  pub const ALL: [Self; 2] = [Self::Abstract, Self::Landscape];

  pub fn label_key(self) -> &'static str {
    match self {
      Self::Abstract => "control_center.wallpaper_category_abstract",
      Self::Landscape => "control_center.wallpaper_category_landscape",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperMode {
  Dark,
  Light,
}

impl WallpaperMode {
  pub const ALL: [Self; 2] = [Self::Dark, Self::Light];

  pub fn label_key(self) -> &'static str {
    match self {
      Self::Dark => "control_center.wallpaper_category_dark",
      Self::Light => "control_center.wallpaper_category_light",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperEntry {
  pub path: String,
  pub collection: WallpaperCollection,
  pub mode: WallpaperMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearancePage {
  Home,
  Themes,
  OfficialThemes,
  ThemeFamilies {
    category: ThemeCategory,
  },
  CustomThemes,
  ThemeImport,
  ThemeImportConfirm,
  ThemeDeleteConfirm,
  ThemeModes {
    family: usize,
  },
  Wallpapers,
  WallpaperModes {
    collection: WallpaperCollection,
  },
  WallpaperItems {
    collection: WallpaperCollection,
    mode: WallpaperMode,
  },
  Accents,
  AccentEdit,
  Effects,
  #[allow(dead_code)]
  Transparency,
  #[allow(dead_code)]
  TransparencySurface {
    surface: EffectSurface,
  },
  #[allow(dead_code)]
  Blur,
  #[allow(dead_code)]
  BlurSurface {
    surface: EffectSurface,
  },
  SpacesBordersPosition,
  TaskbarPosition,
  TaskbarSpaces,
  Taskbar,
  WidgetTelemetry,
  ControlPanel,
  Terminal,
  SurfaceSection {
    surface: EffectSurface,
    section: SurfaceSection,
  },
  WindowSpaces,
  GeneralBorders,
  EdgeThickness,
  Prompt {
    goal: PromptGoal,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectSurface {
  Taskbar,
  ControlPanel,
  WidgetTelemetry,
  Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSection {
  UtilityIcons,
  Sessions,
  Transparency,
  Blur,
}

impl EffectSurface {
  pub const ALL: [Self; 4] = [
    Self::Taskbar,
    Self::ControlPanel,
    Self::WidgetTelemetry,
    Self::Terminal,
  ];

  pub const fn key(self) -> &'static str {
    match self {
      Self::Taskbar => "taskbar",
      Self::ControlPanel => "control-panel",
      Self::WidgetTelemetry => "widget-telemetry",
      Self::Terminal => "terminal",
    }
  }

  pub const fn label_key(self) -> &'static str {
    match self {
      Self::Taskbar => "control_center.taskbar",
      Self::ControlPanel => "control_center.control_panel",
      Self::WidgetTelemetry => "control_center.widget_telemetry",
      Self::Terminal => "control_center.terminal",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Defines `TaskbarPosition`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub enum TaskbarPosition {
  Top,
  Bottom,
}

/// Identifies a separately configurable Widget Telemetry section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetTelemetryBlock {
  System,
  CpuGpu,
  Memory,
  Storage,
  Processes,
  Network,
  Shortcuts,
}

impl WidgetTelemetryBlock {
  pub const ALL: [Self; 7] = [
    Self::System,
    Self::CpuGpu,
    Self::Memory,
    Self::Storage,
    Self::Processes,
    Self::Network,
    Self::Shortcuts,
  ];

  /// Returns the stable command-line identifier owned by the widget package.
  pub fn key(self) -> &'static str {
    match self {
      Self::System => "system",
      Self::CpuGpu => "cpu_gpu",
      Self::Memory => "memory",
      Self::Storage => "storage",
      Self::Processes => "processes",
      Self::Network => "network",
      Self::Shortcuts => "keys",
    }
  }

  /// Returns the localization key for the Control Center row.
  pub fn label_key(self) -> &'static str {
    match self {
      Self::System => "control_center.widget_telemetry_system",
      Self::CpuGpu => "control_center.widget_telemetry_cpu_gpu",
      Self::Memory => "control_center.widget_telemetry_memory",
      Self::Storage => "control_center.widget_telemetry_storage",
      Self::Processes => "control_center.widget_telemetry_processes",
      Self::Network => "control_center.widget_telemetry_network",
      Self::Shortcuts => "control_center.widget_telemetry_shortcuts",
    }
  }
}

/// Stores the sparse Widget Telemetry preferences represented by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetTelemetryBlocks {
  pub system: bool,
  pub cpu_gpu: bool,
  pub memory: bool,
  pub storage: bool,
  pub processes: bool,
  pub network: bool,
  pub shortcuts: bool,
}

impl Default for WidgetTelemetryBlocks {
  fn default() -> Self {
    Self {
      system: true,
      cpu_gpu: true,
      memory: true,
      storage: true,
      processes: true,
      network: true,
      shortcuts: true,
    }
  }
}

impl WidgetTelemetryBlocks {
  pub fn enabled(&self, block: WidgetTelemetryBlock) -> bool {
    match block {
      WidgetTelemetryBlock::System => self.system,
      WidgetTelemetryBlock::CpuGpu => self.cpu_gpu,
      WidgetTelemetryBlock::Memory => self.memory,
      WidgetTelemetryBlock::Storage => self.storage,
      WidgetTelemetryBlock::Processes => self.processes,
      WidgetTelemetryBlock::Network => self.network,
      WidgetTelemetryBlock::Shortcuts => self.shortcuts,
    }
  }

  pub fn set(&mut self, block: WidgetTelemetryBlock, enabled: bool) {
    match block {
      WidgetTelemetryBlock::System => self.system = enabled,
      WidgetTelemetryBlock::CpuGpu => self.cpu_gpu = enabled,
      WidgetTelemetryBlock::Memory => self.memory = enabled,
      WidgetTelemetryBlock::Storage => self.storage = enabled,
      WidgetTelemetryBlock::Processes => self.processes = enabled,
      WidgetTelemetryBlock::Network => self.network = enabled,
      WidgetTelemetryBlock::Shortcuts => self.shortcuts = enabled,
    }
  }
}

/// Identifies a configurable Control Panel card using its stable helper ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPanelCard {
  User,
  Notifications,
  Calendar,
  Weather,
  Volume,
  Brightness,
  Network,
  Bluetooth,
  System,
  Appearance,
  Session,
  Display,
  SpacesBordersPosition,
  Power,
}

impl ControlPanelCard {
  pub const ALL: [Self; 14] = [
    Self::User,
    Self::Notifications,
    Self::Calendar,
    Self::Weather,
    Self::Volume,
    Self::Brightness,
    Self::Network,
    Self::Bluetooth,
    Self::System,
    Self::Appearance,
    Self::Session,
    Self::Display,
    Self::SpacesBordersPosition,
    Self::Power,
  ];

  /// Returns the machine-readable ID owned by the Control Panel helper.
  pub fn key(self) -> &'static str {
    match self {
      Self::User => "user",
      Self::Notifications => "notifications",
      Self::Calendar => "calendar",
      Self::Weather => "weather",
      Self::Volume => "volume",
      Self::Brightness => "brightness",
      Self::Network => "network",
      Self::Bluetooth => "bluetooth",
      Self::System => "system",
      Self::Appearance => "appearance",
      Self::Session => "session",
      Self::Display => "display",
      Self::SpacesBordersPosition => "spaces-borders-position",
      Self::Power => "power",
    }
  }

  /// Returns the shared catalog key for the Control Center row.
  pub fn label_key(self) -> &'static str {
    match self {
      Self::User => "control_center.control_panel_card_user",
      Self::Notifications => "control_center.control_panel_card_notifications",
      Self::Calendar => "control_center.control_panel_card_calendar",
      Self::Weather => "control_center.control_panel_card_weather",
      Self::Volume => "control_center.control_panel_card_volume",
      Self::Brightness => "control_center.control_panel_card_brightness",
      Self::Network => "control_center.control_panel_card_network",
      Self::Bluetooth => "control_center.control_panel_card_bluetooth",
      Self::System => "control_center.control_panel_card_system",
      Self::Appearance => "control_center.control_panel_card_appearance",
      Self::Session => "control_center.control_panel_card_session",
      Self::Display => "control_center.control_panel_card_display",
      Self::SpacesBordersPosition => "control_center.control_panel_card_spaces_borders_position",
      Self::Power => "control_center.control_panel_card_power",
    }
  }
}

/// Stores effective Control Panel visibility while its package owns ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPanelCards {
  enabled: [bool; 14],
  available: [bool; 14],
}

impl Default for ControlPanelCards {
  fn default() -> Self {
    Self {
      enabled: [true; 14],
      available: [true; 14],
    }
  }
}

impl ControlPanelCards {
  pub fn enabled(&self, card: ControlPanelCard) -> bool {
    self.enabled[ControlPanelCard::ALL
      .iter()
      .position(|candidate| *candidate == card)
      .expect("all Control Panel cards have a stable index")]
  }

  pub fn set(&mut self, card: ControlPanelCard, enabled: bool) {
    let index = ControlPanelCard::ALL
      .iter()
      .position(|candidate| *candidate == card)
      .expect("all Control Panel cards have a stable index");
    self.enabled[index] = enabled;
  }

  /// Reports whether the card's required hardware is present on this host.
  pub fn available(&self, card: ControlPanelCard) -> bool {
    self.available[ControlPanelCard::ALL
      .iter()
      .position(|candidate| *candidate == card)
      .expect("all Control Panel cards have a stable index")]
  }

  /// Updates hardware availability while preserving the user's enabled state.
  pub fn set_available(&mut self, card: ControlPanelCard, available: bool) {
    let index = ControlPanelCard::ALL
      .iter()
      .position(|candidate| *candidate == card)
      .expect("all Control Panel cards have a stable index");
    self.available[index] = available;
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Defines the presentation mode for the taskbar utility group.
pub enum TaskbarUtilityGroupMode {
  Auto,
  AlwaysExpanded,
}
impl TaskbarUtilityGroupMode {
  pub fn value(self) -> &'static str {
    match self {
      Self::Auto => "auto",
      Self::AlwaysExpanded => "always-expanded",
    }
  }

  pub fn from_value(value: &str) -> Self {
    if value.trim() == "always-expanded" {
      Self::AlwaysExpanded
    } else {
      Self::Auto
    }
  }
}
impl TaskbarPosition {
  /// Executes the `value` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn value(self) -> &'static str {
    match self {
      Self::Top => "top",
      Self::Bottom => "bottom",
    }
  }
  /// Converts input data into `from_value` while applying local validation. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn from_value(value: &str) -> Self {
    if value.trim() == "bottom" {
      Self::Bottom
    } else {
      Self::Top
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Defines `PromptGoal`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub enum PromptGoal {
  ExportProfile,
  ImportProfile,
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
  /// Executes the `key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn key(self) -> &'static str {
    match self {
      Self::ExportProfile => "export_profile",
      Self::ImportProfile => "import_profile",
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
  /// Executes the const function documented in this module. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
  pub const fn range(self) -> (i32, i32) {
    match self {
      Self::ExportProfile | Self::ImportProfile => (0, 0),
      Self::Rounding => (2, 10),
      Self::Thickness => (0, 10),
      _ => (0, 100),
    }
  }
}

/// Defines the constant `THEMES`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub const THEMES: &[(&str, &str)] = &[
  ("argvus-dark", "ARGVUS Dark"),
  ("argvus-dark-float", "ARGVUS Dark Float"),
  ("dracula", "Dracula"),
  ("dracula-float", "Dracula Float"),
  ("gruvbox-dark", "Gruvbox Dark"),
  ("gruvbox-dark-float", "Gruvbox Dark Float"),
  ("gruvbox-high-dark", "Gruvbox High Dark"),
  ("gruvbox-high-dark-float", "Gruvbox High Dark Float"),
  ("monokai-dark", "Monokai Dark"),
  ("monokai-dark-float", "Monokai Dark Float"),
  ("one-dark", "One Dark"),
  ("one-dark-float", "One Dark Float"),
  ("rose-pine", "Rosé Pine"),
  ("rose-pine-float", "Rosé Pine Float"),
  ("silver-dark", "Silver Dark"),
  ("silver-dark-float", "Silver Dark Float"),
  ("slate-dark", "Slate Dark"),
  ("slate-dark-float", "Slate Dark Float"),
  ("sunset", "Sunset"),
  ("sunset-float", "Sunset Float"),
  ("tokyo-night", "Tokyo-Night"),
  ("tokyo-night-float", "Tokyo-Night Float"),
  ("hackerman", "Hackerman"),
  ("hackerman-float", "Hackerman Float"),
  ("solitude", "Solitude"),
  ("solitude-float", "Solitude Float"),
  ("universe", "Universe"),
  ("universe-float", "Universe Float"),
  ("argvus-light", "ARGVUS Light"),
  ("argvus-light-float", "ARGVUS Light Float"),
  ("catppuccin-latte", "Catppuccin Latte"),
  ("catppuccin-latte-float", "Catppuccin Latte Float"),
  ("frost", "Frost"),
  ("frost-float", "Frost Float"),
  ("github-light", "GitHub Light"),
  ("github-light-float", "GitHub Light Float"),
  ("gruvbox-light", "Gruvbox Light"),
  ("gruvbox-light-float", "Gruvbox Light Float"),
  ("solarized-light", "Solarized Light"),
  ("solarized-light-float", "Solarized Light Float"),
  ("one-light", "One Light"),
  ("one-light-float", "One Light Float"),
  ("everforest-light", "Everforest Light"),
  ("everforest-light-float", "Everforest Light Float"),
];
/// Defines the constant `THEME_FAMILIES`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub const THEME_FAMILIES: &[(&str, &str)] = &[
  ("argvus-dark", "ARGVUS Dark"),
  ("dracula", "Dracula"),
  ("gruvbox-dark", "Gruvbox Dark"),
  ("gruvbox-high-dark", "Gruvbox High Dark"),
  ("monokai-dark", "Monokai Dark"),
  ("one-dark", "One Dark"),
  ("rose-pine", "Rosé Pine"),
  ("silver-dark", "Silver Dark"),
  ("slate-dark", "Slate Dark"),
  ("sunset", "Sunset"),
  ("tokyo-night", "Tokyo-Night"),
  ("hackerman", "Hackerman"),
  ("solitude", "Solitude"),
  ("universe", "Universe"),
  ("argvus-light", "ARGVUS Light"),
  ("catppuccin-latte", "Catppuccin Latte"),
  ("frost", "Frost"),
  ("github-light", "GitHub Light"),
  ("gruvbox-light", "Gruvbox Light"),
  ("solarized-light", "Solarized Light"),
  ("one-light", "One Light"),
  ("everforest-light", "Everforest Light"),
];
/// Executes the `theme_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn theme_label(name: &str) -> String {
  THEMES
    .iter()
    .find(|(code, _)| *code == name)
    .map(|(_, label)| (*label).to_string())
    .unwrap_or_else(|| name.to_string())
}
/// Executes the `theme_family_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn theme_family_label(name: &str) -> String {
  let family = name.strip_suffix("-float").unwrap_or(name);
  THEME_FAMILIES
    .iter()
    .find(|(code, _)| *code == family)
    .map(|(_, label)| (*label).to_string())
    .unwrap_or_else(|| theme_label(family))
}
/// Defines the constant `ACCENTS`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
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
/// Executes the `accent_label` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn accent_label(color: &str) -> String {
  ACCENTS
    .iter()
    .find(|(_, value)| *value == color)
    .map(|(label, _)| (*label).to_string())
    .unwrap_or_else(|| color.to_string())
}

#[derive(Debug, Clone, PartialEq)]
/// Represents `AppearanceState`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct AppearanceState {
  pub theme: String,
  pub accent: String,
  pub wallpapers: Vec<WallpaperEntry>,
  pub wallpaper_active: Option<String>,
  pub animations: bool,
  pub transparency: bool,
  pub blur: bool,
  pub taskbar_transparency_enabled: bool,
  pub control_panel_transparency_enabled: bool,
  pub widget_telemetry_transparency_enabled: bool,
  pub taskbar_blur_enabled: bool,
  pub control_panel_blur_enabled: bool,
  pub widget_telemetry_blur_enabled: bool,
  pub terminal_transparency_enabled: bool,
  pub terminal_blur_enabled: bool,
  pub taskbar_transparency: i32,
  pub control_panel_transparency: i32,
  pub widget_telemetry_transparency: i32,
  pub taskbar_blur: i32,
  pub control_panel_blur: i32,
  pub widget_telemetry_blur: i32,
  pub terminal_transparency: i32,
  pub terminal_blur: i32,
  pub widget_telemetry: bool,
  pub control_panel_enabled: bool,
  pub widget_telemetry_blocks: WidgetTelemetryBlocks,
  pub control_panel_cards: ControlPanelCards,
  pub waybar_pos: TaskbarPosition,
  pub taskbar_utility_group: TaskbarUtilityGroupMode,
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
  pub custom_themes: Vec<CustomTheme>,
  pub active_custom_theme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomTheme {
  pub id: String,
  pub name: String,
  pub base_theme: String,
  pub profile_path: String,
  pub wallpaper_path: Option<String>,
}
impl Default for AppearanceState {
  /// Executes the `default` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn default() -> Self {
    Self {
      theme: "argvus-dark".into(),
      accent: "#3590bd".into(),
      wallpapers: Vec::new(),
      wallpaper_active: None,
      animations: true,
      transparency: true,
      blur: true,
      taskbar_transparency_enabled: true,
      control_panel_transparency_enabled: true,
      widget_telemetry_transparency_enabled: true,
      taskbar_blur_enabled: true,
      control_panel_blur_enabled: true,
      widget_telemetry_blur_enabled: true,
      terminal_transparency_enabled: true,
      terminal_blur_enabled: true,
      taskbar_transparency: 50,
      control_panel_transparency: 50,
      widget_telemetry_transparency: 50,
      taskbar_blur: 50,
      control_panel_blur: 50,
      widget_telemetry_blur: 50,
      terminal_transparency: 50,
      terminal_blur: 50,
      widget_telemetry: true,
      control_panel_enabled: true,
      widget_telemetry_blocks: WidgetTelemetryBlocks::default(),
      control_panel_cards: ControlPanelCards::default(),
      waybar_pos: TaskbarPosition::Top,
      taskbar_utility_group: TaskbarUtilityGroupMode::Auto,
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
      custom_themes: Vec::new(),
      active_custom_theme: None,
    }
  }
}
impl AppearanceState {
  /// Checks the condition represented by `is_float_theme` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn is_float_theme(&self) -> bool {
    self.theme.ends_with("-float")
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  /// Executes the `prompt_metadata_is_explicit` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
  /// Executes the `position_is_type_safe` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn position_is_type_safe() {
    assert_eq!(
      TaskbarPosition::from_value("bottom"),
      TaskbarPosition::Bottom
    );
    assert_eq!(TaskbarPosition::Top.value(), "top");
    assert_eq!(
      TaskbarUtilityGroupMode::from_value("always-expanded"),
      TaskbarUtilityGroupMode::AlwaysExpanded
    );
    assert_eq!(
      TaskbarUtilityGroupMode::from_value("unknown"),
      TaskbarUtilityGroupMode::Auto
    );
    assert_eq!(WidgetTelemetryBlock::CpuGpu.key(), "cpu_gpu");
    assert!(WidgetTelemetryBlocks::default().enabled(WidgetTelemetryBlock::Network));
    assert_eq!(
      ControlPanelCard::SpacesBordersPosition.key(),
      "spaces-borders-position"
    );
    assert!(ControlPanelCards::default().enabled(ControlPanelCard::Power));
  }
  #[test]
  /// Executes the `float_theme_detection` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn float_theme_detection() {
    assert!(
      AppearanceState {
        theme: "argvus-dark-silver-float".into(),
        ..Default::default()
      }
      .is_float_theme()
    );
  }

  #[test]
  fn hex_colors_parse_and_normalize() {
    assert_eq!(
      "#12ABEF".parse::<HexColor>().unwrap().normalized(),
      "#12ABEF"
    );
    assert_eq!(normalize_hex_color("12abef"), Some("#12ABEF".into()));
    assert!(normalize_hex_color("#123").is_none());
    assert!(normalize_hex_color("#12345G").is_none());
    assert!(normalize_hex_color("foo").is_none());
  }

  #[test]
  fn hex_contrast_uses_black_for_light_and_white_for_dark() {
    assert_eq!(
      "#FFFFFF".parse::<HexColor>().unwrap().foreground(),
      ratatui::style::Color::Black
    );
    assert_eq!(
      "#000000".parse::<HexColor>().unwrap().foreground(),
      ratatui::style::Color::White
    );
  }
}
