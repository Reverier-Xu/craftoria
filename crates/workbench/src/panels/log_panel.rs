//! The bottom-dock log panel: streams `tracing` records into a read-only
//! code editor, highlighted in the `tracing_subscriber` console style.
//!
//! The editor supplies virtualization, search, selection, and scrollbar
//! severity markers; the panel only forwards new records from the global log
//! buffer into the console document and keeps the viewport pinned to the
//! latest line.

use std::time::Duration;

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
        .line_number(false)
    });

    cx.spawn_in(window, async move |this, window| {
      loop {
        window.background_executor().timer(POLL_INTERVAL).await;
        if this
          .update_in(window, |this, window, cx| this.sync(window, cx))
          .is_err()
        {
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

  /// Pull new records out of the global buffer, append them to the console
  /// document, and pin the viewport to the latest line.
  fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let drain = logs::global().drain_since(self.cursor);
    self.cursor = drain.cursor;
    if !self.console.append(&drain.lines) {
      return;
    }

    let Some(end) = self.console.end_position() else {
      return;
    };

    // Scrolling to the end goes through the cursor, which grabs focus; save
    // and restore the previously focused element so streaming logs never
    // steal it.
    let previous_focus = window
      .focused(cx)
      .filter(|handle| handle != &self.editor_state.read(cx).focus_handle(cx));
    self.editor_state.update(cx, |state, cx| {
      state.set_cursor_position(end, window, cx);
      cx.notify();
    });
    match previous_focus {
      Some(handle) => window.focus(&handle, cx),
      None => window.blur(cx),
    }
  }
}

impl Panel for LogPanel {
  fn panel_name(&self) -> &'static str {
    Self::PANEL_NAME
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some("Logs".into())
  }

  fn title(&self, _cx: &App) -> SharedString {
    "Logs".into()
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
    div()
      .size_full()
      .bg(cx.theme().editor_background)
      .child(CodeEditor::new(&self.editor_state).h_full())
  }
}
