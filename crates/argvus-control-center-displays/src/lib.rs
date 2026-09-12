mod backend;
mod model;
mod ui;

pub use model::{
  DisplayPage, DisplayState, Mode, Monitor, MonitorInfo, MonitorProfile, MonitorSetting,
  PersistedConfig, PersistedMonitor, PromptGoal,
};
pub use ui::DisplaysApp;