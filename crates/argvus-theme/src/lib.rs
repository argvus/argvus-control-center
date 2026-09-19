//! Root module of crate `argvus theme`. It exposes public boundaries and composes internal responsibilities without duplicating domain rules.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use ratatui::style::Color;

pub mod fallback;
pub mod loader;
pub mod parser;
pub mod resolver;

#[derive(Debug, Clone)]
/// Represents `Theme`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Theme {
  pub name: String,
  pub is_light: bool,
  pub background: Color,
  pub foreground: Color,
  pub accent: Color,
  pub tab_active: Color,
  pub tab_inactive: Color,
  pub selected_background: Color,
  pub selected_foreground: Color,
  pub border: Color,
  pub border_active: Color,
  pub muted: Color,
  pub link: Color,
  pub success: Color,
  pub warning: Color,
  pub error: Color,
  pub surface: Color,
  pub accent_alpha: Color,
}

impl Theme {
  /// Retrieves data for `load` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn load() -> Self {
    resolver::resolve(&loader::Loader::new())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  /// Retrieves data for `load_always_produces_a_readable_theme` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn load_always_produces_a_readable_theme() {
    let theme = Theme::load();
    assert!(!theme.name.is_empty());
    assert_ne!(theme.background, theme.foreground);
  }
}
