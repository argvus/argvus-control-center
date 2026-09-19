//! Implements shared button rendering in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
pub use argvus_tui::buttons::{Button, ButtonKind, draw, height};
