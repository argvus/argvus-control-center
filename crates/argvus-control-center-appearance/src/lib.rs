//! Root module of crate `argvus control center appearance`. It exposes public boundaries and composes internal responsibilities without duplicating domain rules.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
mod backend;
mod model;
mod profile;
mod ui;

pub use model::{
  ACCENTS, AppearancePage, AppearanceState, ControlPanelCard, ControlPanelCards, CustomTheme,
  PromptGoal, THEME_FAMILIES, THEMES, TaskbarPosition, TaskbarUtilityGroupMode, ThemeCategory,
  WallpaperCollection, WallpaperEntry, WallpaperMode, WidgetTelemetryBlock, WidgetTelemetryBlocks,
};
pub use ui::AppearanceApp;
