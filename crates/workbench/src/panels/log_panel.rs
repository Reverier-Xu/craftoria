//! The bottom-dock log panel: streams `tracing` records live from the global
//! log buffer into a virtualized, auto-scrolling list.

use std::{rc::Rc, time::Duration};

use tracing::Level;
use woocraft::{
  ActiveTheme, IconName, Panel, PanelEvent, Size, StyleSized as _, StyledExt as _,
  TERMINAL_FONT_FAMILY,
  gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, Hsla, IntoElement, ParentElement, Render,
    ScrollStrategy, SharedString, Styled, UniformListScrollHandle, Window, div,
    prelude::FluentBuilder as _, uniform_list,
  },
  h_flex, v_flex,
};

use crate::logs::{self, LogLine};

/// How often the panel polls the log buffer for new records.
const POLL_INTERVAL: Duration = Duration::from_millis(120);

/// Log rows follow the standard component tier of the design system.
///
/// All text renders at the unified `1em` size; emphasis is expressed through
/// font weight and opacity, never through smaller font sizes.
const ROW_SIZE: Size = Size::Medium;

/// Per-level presentation, resolved from the active theme.
#[derive(Clone, Copy)]
struct LineColors {
  level: Hsla,
  message: Hsla,
}

/// A dock panel that renders the streamed log records.
pub struct LogPanel {
  lines: Rc<Vec<LogLine>>,
  cursor: u64,
  scroll_handle: UniformListScrollHandle,
  focus_handle: FocusHandle,
}

impl LogPanel {
  pub const PANEL_NAME: &'static str = "craftoria.logs";

  pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
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
      lines: Rc::new(Vec::new()),
      cursor: 0,
      scroll_handle: UniformListScrollHandle::new(),
      focus_handle: cx.focus_handle(),
    }
  }

  /// Pull new records out of the global buffer and keep the view pinned to
  /// the latest line.
  fn sync(&mut self, cx: &mut Context<Self>) {
    let drain = logs::global().drain_since(self.cursor);
    if drain.lines.is_empty() {
      return;
    }
    if drain.reset {
      self.lines = Rc::new(drain.lines);
    } else {
      Rc::make_mut(&mut self.lines).extend(drain.lines);
    }
    self.cursor = drain.cursor;
    self
      .scroll_handle
      .scroll_to_item(self.lines.len().saturating_sub(1), ScrollStrategy::Bottom);
    cx.notify();
  }

  fn level_colors(level: Level, cx: &App) -> LineColors {
    let theme = cx.theme();
    match level {
      Level::ERROR => LineColors {
        level: theme.danger,
        message: theme.danger,
      },
      Level::WARN => LineColors {
        level: theme.warning,
        message: theme.foreground,
      },
      Level::INFO => LineColors {
        level: theme.success,
        message: theme.foreground,
      },
      Level::DEBUG | Level::TRACE => LineColors {
        level: theme.muted_foreground,
        message: theme.muted_foreground,
      },
    }
  }

  fn render_line(line: &LogLine, muted: Hsla, colors: LineColors) -> impl IntoElement + use<> {
    h_flex()
      .w_full()
      .component_h(ROW_SIZE)
      .component_px(ROW_SIZE)
      .component_gap(ROW_SIZE)
      .items_center()
      .line_height(ROW_SIZE.text_size())
      .child(
        div()
          .w(ROW_SIZE.em(5.5))
          .flex_shrink_0()
          .text_color(muted)
          .child(format_elapsed(line.elapsed)),
      )
      .child(
        div()
          .w(ROW_SIZE.em(3.5))
          .flex_shrink_0()
          .font_semibold()
          .text_color(colors.level)
          .child(line.level.as_str()),
      )
      .child(
        div()
          .flex_shrink_0()
          .text_color(muted)
          .child(line.target.clone()),
      )
      .child(
        div()
          .whitespace_nowrap()
          .text_color(colors.message)
          .child(line.message.clone()),
      )
  }
}

/// Format an elapsed duration as `mm:ss.mmm`.
fn format_elapsed(elapsed: Duration) -> String {
  let seconds = elapsed.as_secs();
  format!(
    "{:02}:{:02}.{:03}",
    seconds / 60,
    seconds % 60,
    elapsed.subsec_millis()
  )
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
    let muted = cx.theme().muted_foreground;
    let lines = self.lines.clone();

    v_flex()
      .size_full()
      .bg(cx.theme().background)
      .font_family(TERMINAL_FONT_FAMILY)
      .when(lines.is_empty(), |this| {
        this
          .items_center()
          .justify_center()
          .child(div().text_color(muted).child("Waiting for log output…"))
      })
      .when(!lines.is_empty(), |this| {
        this.child(
          uniform_list(
            "craftoria-log-lines",
            lines.len(),
            move |range, _window, cx| {
              range
                .map(|index| {
                  let colors = Self::level_colors(lines[index].level, cx);
                  Self::render_line(&lines[index], muted, colors)
                })
                .collect()
            },
          )
          .track_scroll(&self.scroll_handle)
          .size_full(),
        )
      })
  }
}
