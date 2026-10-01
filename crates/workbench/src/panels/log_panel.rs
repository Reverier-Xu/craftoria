//! The bottom-dock log panel: streams `tracing` records into a read-only
//! code editor, highlighted in the `tracing_subscriber` console style.
//!
//! The editor supplies virtualization, incremental re-wrapping of appended
//! batches, selection, search, and scrollbar severity markers; the panel
//! only forwards new records from the global log buffer into the console
//! document. Follow-output scrolling (pause on scroll-up, re-arm at the
//! bottom) is handled by the editor itself.

use std::time::Duration;

use craftoria_exthost::i18n as exthost_i18n;
use woocraft::{
  ActiveTheme, CodeEditor, EditorState, IconName, Panel, PanelEvent,
  gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div,
  },
};

use crate::{
  log_console::{LogConsole, LogConsoleBackend},
  logs,
};

/// How often the panel polls the log buffer for new records.
const POLL_INTERVAL: Duration = Duration::from_millis(120);

/// A dock panel that renders the streamed log records in a read-only editor.
pub struct LogPanel {
  console: LogConsole,
  editor_state: Entity<EditorState>,
  cursor: u64,
  focus_handle: FocusHandle,
}

impl LogPanel {
  pub const PANEL_NAME: &'static str = "craftoria.logs";

  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let console = LogConsole::default();
    let editor_state = cx.new(|cx| {
      EditorState::new(window, cx)
        .code_editor("log")
        .backend(LogConsoleBackend::new(console.clone()))
        .read_only(true)
        .follow_output(true)
        .line_number(false)
    });

    cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor().timer(POLL_INTERVAL).await;
        if this.update(cx, |this, cx| this.sync(cx)).is_err() {
          break;
        }
      }
    })
    .detach();

    Self {
      console,
      editor_state,
      cursor: 0,
      focus_handle: cx.focus_handle(),
    }
  }

  /// Pull new records out of the global buffer and append them to the
  /// console document. The editor picks the changes up on its next render
  /// (revision bump) and keeps the viewport pinned to the tail via
  /// follow-output.
  fn sync(&mut self, cx: &mut Context<Self>) {
    let drain = logs::global().drain_since(self.cursor);
    self.cursor = drain.cursor;
    if self.console.append(&drain.lines) {
      cx.notify();
    }
  }
}

impl Panel for LogPanel {
  fn panel_name(&self) -> &'static str {
    Self::PANEL_NAME
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(exthost_i18n::tr_static("panel.logs.title"))
  }

  fn title(&self, _cx: &App) -> SharedString {
    exthost_i18n::tr_static("panel.logs.title")
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Prompt
  }
}

impl EventEmitter<PanelEvent> for LogPanel {}

impl Focusable for LogPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for LogPanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div().size_full().bg(cx.theme().editor_background).child(
      CodeEditor::new(&self.editor_state)
        .h_full()
        .w_full()
        .appearance(false)
        .bordered(false)
        .focus_bordered(false),
    )
  }
}
