//! Dock panels of the workbench.

mod log_panel;
mod placeholder;

pub use log_panel::LogPanel;
pub use placeholder::PlaceholderPanel;
use woocraft::{
  gpui::{App, AppContext as _},
  register_panel,
};

/// Register every panel type so saved dock layouts can be restored.
///
/// Registration keys are the stable `Panel::panel_name` values; they must
/// never change once a layout containing them has been persisted.
pub(crate) fn register(cx: &mut App) {
  register_panel(
    cx,
    LogPanel::PANEL_NAME,
    |_dock, _state, _info, window, cx| Box::new(cx.new(|cx| LogPanel::new(window, cx))),
  );
  register_panel(
    cx,
    PlaceholderPanel::EXPLORER_NAME,
    |_dock, _state, _info, _window, cx| Box::new(cx.new(PlaceholderPanel::explorer)),
  );
  register_panel(
    cx,
    PlaceholderPanel::WELCOME_NAME,
    |_dock, _state, _info, _window, cx| Box::new(cx.new(PlaceholderPanel::welcome)),
  );
}
