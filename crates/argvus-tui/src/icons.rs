//! Implements shared icon catalog in crate `argvus tui`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! Icons use the Material Design Icons section of Nerd Fonts (`nf-md-*`).
//! Keep constants to one glyph with no layout whitespace; spacing belongs to
//! [`icon_label`]. The recommended terminal font is Symbols Nerd Font Mono.

/// Fixed gap between the icon cell and its label.
pub const ICON_TEXT_GAP: &str = " ";

/// Render a consistently spaced icon/label pair for legacy string-based rows.
pub fn icon_label(icon: &str, label: impl AsRef<str>) -> String {
  if icon.is_empty() {
    label.as_ref().to_owned()
  } else {
    format!("{icon}{ICON_TEXT_GAP}{}", label.as_ref())
  }
}

/// Nerd Fonts: nf-md-network
pub const NETWORK: &str = "\u{f06f1}";
/// Nerd Fonts: nf-md-wifi
pub const WIFI: &str = "\u{f0928}";
/// Nerd Fonts: nf-md-ethernet
pub const ETHERNET: &str = "\u{f0200}";
/// Nerd Fonts: nf-md-vpn
pub const VPN: &str = "\u{f0583}";
/// Nerd Fonts: nf-md-dns
pub const DNS: &str = "\u{f0155}";
/// Nerd Fonts: nf-md-magnify
pub const SEARCH: &str = "\u{f0349}";
/// Nerd Fonts: nf-md-bluetooth
pub const BLUETOOTH: &str = "\u{f00af}";
/// Nerd Fonts: nf-md-volume-high
pub const AUDIO: &str = "\u{f057e}";
/// Nerd Fonts: nf-md-speaker
pub const SPEAKER: &str = "\u{f04c3}";
/// Nerd Fonts: nf-md-microphone
pub const MICROPHONE: &str = "\u{f036c}";
/// Nerd Fonts: nf-md-chip
pub const HARDWARE: &str = "\u{f061a}";
/// Nerd Fonts: nf-md-cpu-64-bit
pub const CPU: &str = "\u{f0ee0}";
/// Nerd Fonts: nf-md-expansion-card
pub const GPU: &str = "\u{f0fb2}";
/// Nerd Fonts: nf-md-memory
pub const MEMORY: &str = "\u{f035b}";
/// Nerd Fonts: nf-md-power
pub const POWER: &str = "\u{f0425}";
/// Nerd Fonts: nf-md-cog
pub const SETTINGS: &str = "\u{f0493}";
/// Nerd Fonts: nf-md-server
pub const SERVICES: &str = "\u{f048b}";
/// Nerd Fonts: nf-md-account
pub const USER: &str = "\u{f0004}";
/// Nerd Fonts: nf-md-text-box
pub const LOGS: &str = "\u{f021a}";
/// Nerd Fonts: nf-md-alert
pub const WARNING: &str = "\u{f0026}";
/// Nerd Fonts: nf-md-boot
pub const BOOT: &str = "\u{f0064}";
/// Nerd Fonts: nf-md-package-variant
pub const PACKAGES: &str = "\u{f03d7}";
/// Nerd Fonts: nf-md-package-check
pub const INSTALLED: &str = "\u{f03d9}";
/// Nerd Fonts: nf-md-update
pub const UPDATE: &str = "\u{f06b0}";
/// Nerd Fonts: nf-md-history
pub const HISTORY: &str = "\u{f02da}";
/// Nerd Fonts: nf-md-harddisk
pub const STORAGE: &str = "\u{f02ca}";
/// Nerd Fonts: nf-md-chart-box
pub const DIAGNOSTICS: &str = "\u{f0151}";
/// Nerd Fonts: nf-md-apps
pub const APPS: &str = "\u{f003b}";
/// Nerd Fonts: nf-md-format-font
pub const FONTS: &str = "\u{f019f}";
/// Nerd Fonts: nf-md-information
pub const INFO: &str = "\u{f02fc}";
/// Nerd Fonts: nf-md-check-circle
pub const SUCCESS: &str = "\u{f05e0}";
/// Nerd Fonts: nf-md-close-circle
pub const ERROR: &str = "\u{f0159}";
/// Nerd Fonts: nf-md-refresh
pub const REFRESH: &str = "\u{f0450}";
/// Nerd Fonts: nf-md-link
pub const LINK: &str = "\u{f0337}";
/// Nerd Fonts: nf-md-monitor
pub const MONITOR: &str = "\u{f0379}";
/// Nerd Fonts: nf-md-keyboard
pub const KEYBOARD: &str = "\u{f030c}";
/// Nerd Fonts: nf-md-image
pub const IMAGE: &str = "\u{f02f9}";
/// Nerd Fonts: nf-md-palette
pub const PALETTE: &str = "\u{f03d8}";
/// Nerd Fonts: nf-md-mouse
pub const MOUSE: &str = "\u{f037d}";
/// Nerd Fonts: nf-md-folder
pub const FOLDER: &str = "\u{f024b}";
/// Nerd Fonts: nf-md-lock
pub const LOCK: &str = "\u{f033e}";
/// Nerd Fonts: nf-md-battery
pub const BATTERY: &str = "\u{f0079}";
/// Nerd Fonts: nf-md-account-multiple
pub const USERS: &str = "\u{f000d}";
/// Nerd Fonts: nf-md-bell
pub const BELL: &str = "\u{f009c}";
/// Nerd Fonts: nf-md-bell-off
pub const BELL_OFF: &str = "\u{f009e}";
/// Nerd Fonts: nf-md-form-textbox
pub const TEXT_EDITOR: &str = "\u{f060e}";
/// Nerd Fonts: nf-md-file-pdf-box
pub const PDF: &str = "\u{f0e2d}";
/// Nerd Fonts: nf-md-video
pub const VIDEO: &str = "\u{f0567}";
/// Nerd Fonts: nf-md-music
pub const MUSIC: &str = "\u{f075a}";
/// Nerd Fonts: nf-md-clock-outline
pub const CLOCK: &str = "\u{f0150}";
/// Nerd Fonts: nf-md-satellite-variant
pub const SATELLITE: &str = "\u{f0471}";
/// Nerd Fonts: nf-md-ruler
pub const RULER: &str = "\u{f046d}";
/// Nerd Fonts: nf-md-auto-fix
pub const EFFECT: &str = "\u{f0068}";

#[cfg(test)]
mod tests {
  use super::*;

  /// Defines the constant `ALL`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
  const ALL: &[&str] = &[
    NETWORK,
    WIFI,
    ETHERNET,
    VPN,
    DNS,
    SEARCH,
    BLUETOOTH,
    AUDIO,
    SPEAKER,
    MICROPHONE,
    HARDWARE,
    CPU,
    GPU,
    MEMORY,
    POWER,
    SETTINGS,
    SERVICES,
    USER,
    LOGS,
    WARNING,
    BOOT,
    PACKAGES,
    INSTALLED,
    UPDATE,
    HISTORY,
    STORAGE,
    DIAGNOSTICS,
    APPS,
    FONTS,
    INFO,
    SUCCESS,
    ERROR,
    REFRESH,
    LINK,
    MONITOR,
    KEYBOARD,
    IMAGE,
    PALETTE,
    MOUSE,
    FOLDER,
    LOCK,
    BATTERY,
    USERS,
    TEXT_EDITOR,
    PDF,
    VIDEO,
    MUSIC,
    CLOCK,
    SATELLITE,
    RULER,
    EFFECT,
    BELL,
    BELL_OFF,
  ];

  #[test]
  /// Executes the `catalog_entries_are_single_non_whitespace_glyphs` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn catalog_entries_are_single_non_whitespace_glyphs() {
    for icon in ALL {
      assert!(!icon.is_empty());
      assert_eq!(icon.trim(), *icon);
      assert!(!icon.chars().any(char::is_whitespace));
      assert_eq!(crate::text::display_width(icon), 1, "{icon:?}");
    }
  }

  #[test]
  /// Executes the `icon_label_has_stable_geometry_and_safe_narrow_widths` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn icon_label_has_stable_geometry_and_safe_narrow_widths() {
    assert_eq!(icon_label(WIFI, "Network"), format!("{WIFI} Network"));
    assert_eq!(crate::text::display_width(&icon_label(WIFI, "Network")), 9);
    assert_eq!(icon_label("", "Network"), "Network");
  }
}
