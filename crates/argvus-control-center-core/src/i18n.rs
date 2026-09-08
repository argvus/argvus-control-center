//! Minimal ARGVUS localization shared with argvus-default-apps.
//!
//! Locale precedence intentionally matches `argvus-default-apps`:
//! `LC_ALL` > `LC_MESSAGES` > `LANG` > `LANGUAGE`. Portuguese is used when
//! the selected locale starts with `pt`; English is the default fallback.

pub fn is_pt() -> bool {
  let locale = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
    .iter()
    .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
  locale
    .map(|l| l.to_lowercase().starts_with("pt"))
    .unwrap_or(false)
}

pub fn label<'a>(pt_text: &'a str, en_text: &'a str) -> &'a str {
  if is_pt() { pt_text } else { en_text }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn label_picks_supported_locale() {
    unsafe {
      std::env::set_var("LC_ALL", "pt_BR.UTF-8");
    }
    assert_eq!(label("Aplicar", "Apply"), "Aplicar");

    unsafe {
      std::env::set_var("LC_ALL", "en_US.UTF-8");
    }
    assert_eq!(label("Aplicar", "Apply"), "Apply");

    unsafe {
      std::env::set_var("LC_ALL", "de_DE.UTF-8");
    }
    assert_eq!(label("Aplicar", "Apply"), "Apply");

    unsafe {
      std::env::remove_var("LC_ALL");
      std::env::remove_var("LC_MESSAGES");
      std::env::remove_var("LANG");
      std::env::remove_var("LANGUAGE");
    }
  }
}
