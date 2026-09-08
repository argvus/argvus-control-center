use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
  En,
  Pt,
}

impl Lang {
  pub fn detect() -> Self {
    if let Ok(value) = env::var("ARGVUS_LANG") {
      return from_locale(&value);
    }
    ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
      .iter()
      .find_map(|key| env::var(key).ok().filter(|value| !value.is_empty()))
      .map_or(Self::En, |value| from_locale(&value))
  }
}

fn from_locale(value: &str) -> Lang {
  if value.to_ascii_lowercase().starts_with("pt") {
    Lang::Pt
  } else {
    Lang::En
  }
}

pub const fn tr<'a>(lang: Lang, pt: &'a str, en: &'a str) -> &'a str {
  match lang {
    Lang::Pt => pt,
    Lang::En => en,
  }
}

pub const fn na(lang: Lang) -> &'static str {
  tr(lang, "N/D", "N/A")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn translates_every_supported_language() {
    assert_eq!(tr(Lang::Pt, "Voltar", "Back"), "Voltar");
    assert_eq!(tr(Lang::En, "Voltar", "Back"), "Back");
    assert_eq!(na(Lang::Pt), "N/D");
  }

  #[test]
  fn parses_argvus_language_values() {
    assert_eq!(from_locale("pt_BR.UTF-8"), Lang::Pt);
    assert_eq!(from_locale("en_US.UTF-8"), Lang::En);
  }
}
