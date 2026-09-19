//! Implements shared filesystem path resolution in crate `argvus control center core`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use std::env;
use std::path::PathBuf;

/// Executes the `home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn home() -> PathBuf {
  env::var_os("HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Executes the `config_home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn config_home() -> PathBuf {
  env::var_os("XDG_CONFIG_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| home().join(".config"))
}

/// Executes the `argvus_config_home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn argvus_config_home() -> PathBuf {
  env::var_os("ARGVUS_CONFIG_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(config_home)
    .join("argvus")
}

/// Executes the `legacy_argvus_state_home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn legacy_argvus_state_home() -> PathBuf {
  let base = env::var_os("XDG_STATE_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| home().join(".local").join("state"));
  base.join("argvus")
}

/// Executes the `active_argvus_theme_file` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn active_argvus_theme_file() -> PathBuf {
  argvus_config_home().join(".active-theme")
}

/// Executes the `cache_home` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn cache_home() -> PathBuf {
  env::var_os("XDG_CACHE_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| home().join(".cache"))
}

/// Executes the `defaults_file` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn defaults_file() -> PathBuf {
  argvus_config_home().join("defaults.json")
}

/// Executes the `fonts_file` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn fonts_file() -> PathBuf {
  argvus_config_home().join("fonts.conf")
}

/// Executes the `system_config_root` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn system_config_root() -> PathBuf {
  env::var_os("ARGVUS_SYSTEM_CONFIG")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("/usr/share/argvus"))
}

/// Executes the `legacy_defaults_file` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn legacy_defaults_file() -> PathBuf {
  legacy_argvus_state_home().join("defaults.json")
}

/// Executes the `mimeapps_list` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn mimeapps_list() -> PathBuf {
  config_home().join("mimeapps.list")
}

/// Executes the `user_applications_dir` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn user_applications_dir() -> PathBuf {
  env::var_os("XDG_DATA_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| home().join(".local").join("share"))
    .join("applications")
}

/// Executes the `system_applications_dirs` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn system_applications_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  if let Some(list) = env::var_os("XDG_DATA_DIRS") {
    for d in env::split_paths(&list) {
      if !d.as_os_str().is_empty() {
        dirs.push(d);
      }
    }
  }
  if dirs.is_empty() {
    dirs.push(PathBuf::from("/usr/local/share"));
    dirs.push(PathBuf::from("/usr/share"));
  }
  dirs.into_iter().map(|d| d.join("applications")).collect()
}
