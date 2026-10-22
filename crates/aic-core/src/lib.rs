//! Núcleo do AIC, sem UI: modelo, store, captura rápida, git, ponte com o Claude Code.

pub mod claude;
pub mod config;
pub mod events;
pub mod git;
pub mod inbox;
pub mod model;
pub mod modules;
pub mod paths;
pub mod quickadd;
pub mod store;
pub mod usage;
pub mod voice;

pub use config::{Config, LayneConfig, SessionView, Transcriber, VoiceConfig};
pub use modules::Module;
pub use model::*;
pub use store::Store;
