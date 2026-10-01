#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! The craftoria desktop workbench: the application framework behind the
//! `craftoria` binary.
//!
//! The workbench composes a title bar, a dock layout, and a status bar around
//! dockable panels. Log records emitted through `tracing` are captured into an
//! in-memory ring buffer (see [`logs`]) and streamed live into the bottom-dock
//! log panel.

pub mod app;
pub mod logs;
pub mod panels;

mod error;
mod layout;
mod log_console;
mod paths;
mod settings;
mod translations;

pub use app::{GuiOptions, run};
pub use error::WorkbenchError;
pub use settings::Settings;
