//! Root module of crate `argvus control center appearance`. It exposes public boundaries and composes internal responsibilities without duplicating domain rules.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
mod backend;
mod model;
mod ui;

pub use model::{
  ACCENTS, AppearancePage, AppearanceState, PromptGoal, THEME_FAMILIES, THEMES, TaskbarPosition,
};
pub use ui::AppearanceApp;
