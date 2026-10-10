//! Núcleo do AIC, sem UI: modelo, store, captura rápida, git, ponte com o Claude Code.

pub mod card;
pub mod claude;
pub mod config;
pub mod events;
pub mod git;
pub mod model;
pub mod modules;
pub mod notion;
pub mod paths;
pub mod quickadd;
pub mod store;
pub mod usage;

pub use config::{Config, Connector, SessionView};
pub use modules::Module;
pub use model::*;
pub use store::Store;
