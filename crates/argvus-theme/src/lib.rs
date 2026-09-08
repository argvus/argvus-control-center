use ratatui::style::Color;

pub mod fallback;
pub mod loader;
pub mod parser;
pub mod resolver;

#[derive(Debug, Clone)]
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
  pub fn load() -> Self {
    resolver::resolve(&loader::Loader::new())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn load_always_produces_a_readable_theme() {
    let theme = Theme::load();
    assert!(!theme.name.is_empty());
    assert_ne!(theme.background, theme.foreground);
  }
}
