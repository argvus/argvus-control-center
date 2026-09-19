//! Implements module declarations for the `system` subsystem in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
pub mod command;
pub mod fonts;
pub mod host;
pub mod input;
pub mod keybindings;
pub mod keyboard;
pub mod locale;
pub mod privileged;
pub mod ratbag;
pub mod time;
