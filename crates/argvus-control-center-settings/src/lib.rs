pub mod administration;
pub mod app;
pub mod config;
pub mod error;
pub mod event;
pub mod navigation;
pub mod system;
pub mod ui;

mod i18n {
  pub use argvus_i18n::*;
}

mod theme {
  pub use argvus_theme::*;
}

pub use app::App;
pub use argvus_control_center_apps::catalog::Category;
pub use config::fonts::{FontTarget, SettingKind};
pub use error::SettingsError;
pub use navigation::Page;
