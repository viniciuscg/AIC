//! Terminal embutido do AIC: PTY + alacritty_terminal + widget egui.
//! Base derivada do egui_term (MIT), reescrita para egui 0.36.

mod backend;
pub mod fonts;
mod bindings;
mod theme;
mod view;

pub use alacritty_terminal::term::TermMode as TerminalMode;
pub use backend::{GridSize, Shared, TermSettings, TerminalBackend, now_ms};
pub use bindings::{Binding, BindingAction, BindingsLayout, InputKind, KeyboardBinding};
pub use theme::{TerminalTheme, hex};
pub use view::{TerminalFont, TerminalView};
