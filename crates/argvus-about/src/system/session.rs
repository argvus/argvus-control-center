use std::env;

pub fn session_name_from(kind: Option<&str>, desktop: Option<&str>) -> Option<String> {
  match (kind, desktop) {
    (Some(kind), Some(desktop)) if !desktop.is_empty() => Some(format!("{kind} ({desktop})")),
    (Some(kind), _) => Some(kind.to_string()),
    (_, Some(desktop)) if !desktop.is_empty() => Some(desktop.to_string()),
    _ => None,
  }
}

pub fn session_name() -> Option<String> {
  let kind = env::var("XDG_SESSION_TYPE").ok();
  let desktop = env::var("XDG_CURRENT_DESKTOP")
    .or_else(|_| env::var("XDG_SESSION_DESKTOP"))
    .ok();
  session_name_from(kind.as_deref(), desktop.as_deref())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn combines_session_type_and_desktop() {
    assert_eq!(
      session_name_from(Some("wayland"), Some("Hyprland")).as_deref(),
      Some("wayland (Hyprland)")
    );
  }

  #[test]
  fn handles_partial_values() {
    assert_eq!(
      session_name_from(Some("wayland"), None).as_deref(),
      Some("wayland")
    );
    assert_eq!(
      session_name_from(None, Some("Hyprland")).as_deref(),
      Some("Hyprland")
    );
    assert_eq!(session_name_from(None, None), None);
  }
}
