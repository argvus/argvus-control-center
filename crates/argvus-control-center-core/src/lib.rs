//! Root module of crate `argvus control center core`. It exposes public boundaries and composes internal responsibilities without duplicating domain rules.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
pub mod capabilities;
pub mod config;
pub mod i18n;
pub mod jobs;
pub mod paths;
pub mod privileged;
pub mod process;
pub mod sanitize;
pub mod search;
