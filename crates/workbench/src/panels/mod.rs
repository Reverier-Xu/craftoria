//! Dock panels the workbench shell itself renders.
//!
//! Everything here is contributed through [`crate::extension`] — this
//! module only hosts the panel types.

mod log_panel;
mod placeholder;

pub use log_panel::LogPanel;
pub use placeholder::PlaceholderPanel;
