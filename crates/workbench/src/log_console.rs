//! A tracing-console styled editor backend for the log panel.
//!
//! [`LogConsole`] owns the log document (a rope capped at [`MAX_LINES`]
//! lines), exposes it to the editor through the [`EditorBackend`] interface,
//! and provides [`TracingConsoleHighlighter`], a syntax highlighter that
//! renders records in the style of `tracing_subscriber`'s console output:
//!
//! ```text
//! 00:12.345  INFO craftoria_workbench::app: main window opened
//! ^^^^^^^^^  ^^^^ ^^^^^^^^^^^^^^^^^^^^^^^^ ^^^^^^^^^^^^^^^^^^
//! timestamp  level       target               message
//! ```
//!
//! The timestamp and target are dimmed, the level is colored and bold
//! (ERROR red, WARN orange, INFO green, DEBUG blue, TRACE purple — the hues
//! `tracing_subscriber::fmt` uses), and the message keeps the default text
//! color. Colors resolve through the active syntax theme so light/dark
//! switching keeps working.

use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc, time::Duration};

use tracing::Level;
use woocraft::{
  EditorBackend, EditorHighlighter, EditorSnapshot, HighlightTheme, Position, Rope,
  RopeEditorSnapshot, RopeExt as _, ScrollbarMarker,
  gpui::{FontWeight, HighlightStyle, rgb},
};

use crate::logs::LogLine;

/// Maximum number of lines kept in the console document; the oldest lines
/// are dropped first.
const MAX_LINES: u64 = 10_000;

/// Color of the scrollbar marker for error records (VS Code editorError red).
const MARKER_ERROR: u32 = 0xF14C4C;
/// Color of the scrollbar marker for warning records (VS Code
/// editorWarning yellow).
const MARKER_WARN: u32 = 0xCCA700;

/// Render one record in `tracing_subscriber::fmt` console layout.
fn render_record(line: &LogLine) -> String {
  format!(
    "{} {:>5} {}: {}\n",
    format_elapsed(line.elapsed),
    line.level.as_str(),
    line.target,
    line.message
  )
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

/// What a highlighted span of a console line means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpanKind {
  Timestamp,
  Level(Level),
  Target,
}

/// Parse one rendered console line (without the trailing newline) into
/// styled spans with line-relative byte ranges.
///
/// Returns an empty vector for lines that do not match the console layout
/// (for example continuation lines of a multi-line message); they render
/// unstyled.
fn parse_line(text: &str) -> Vec<(Range<u64>, SpanKind)> {
  let timestamp_end = text.find(' ').unwrap_or(0) as u64;
  if timestamp_end == 0 {
    return Vec::new();
  }

  // Levels are padded to width 5, so skip the padding before the word.
  let rest = &text[timestamp_end as usize..];
  let Some(padding) = rest.find(|c: char| !c.is_whitespace()) else {
    return Vec::new();
  };
  let level_start = timestamp_end as usize + padding;
  let Some(width) = rest[padding..].find(char::is_whitespace) else {
    return Vec::new();
  };
  let level_end = level_start + width;
  let level = match &text[level_start..level_end] {
    "ERROR" => Level::ERROR,
    "WARN" => Level::WARN,
    "INFO" => Level::INFO,
    "DEBUG" => Level::DEBUG,
    "TRACE" => Level::TRACE,
    _ => return Vec::new(),
  };

  let mut spans = vec![
    (0..timestamp_end, SpanKind::Timestamp),
    (level_start as u64..level_end as u64, SpanKind::Level(level)),
  ];

  // The target ends at the first ": " after the level.
  if let Some(separator) = text[level_end..].find(": ") {
    let target_end = level_end + separator + 1;
    spans.push((level_end as u64..target_end as u64, SpanKind::Target));
  }

  spans
}

/// The shared log document, cloned between the panel (writer) and the
/// editor backend (reader).
#[derive(Clone, Default)]
pub(crate) struct LogConsole {
  inner: Rc<RefCell<Inner>>,
}

#[derive(Default)]
struct Inner {
  text: Rope,
  revision: u64,
  line_count: u64,
  markers: Vec<ScrollbarMarker>,
}

impl LogConsole {
  /// Append records to the document; returns `true` when the document
  /// changed.
  pub(crate) fn append(&self, lines: &[LogLine]) -> bool {
    if lines.is_empty() {
      return false;
    }

    let mut inner = self.inner.borrow_mut();

    let mut chunk = String::new();
    for line in lines {
      if line.level == Level::ERROR || line.level == Level::WARN {
        let row = inner.line_count + chunk.matches('\n').count() as u64;
        let color = if line.level == Level::ERROR {
          rgb(MARKER_ERROR).into()
        } else {
          rgb(MARKER_WARN).into()
        };
        inner.markers.push(ScrollbarMarker::dot(row, color));
      }
      chunk.push_str(&render_record(line));
    }

    let new_lines = chunk.matches('\n').count() as u64;
    let end = inner.text.len();
    inner.text.insert(end, &chunk);
    inner.line_count += new_lines;

    let overflow = inner.line_count.saturating_sub(MAX_LINES);
    if overflow > 0 {
      inner.truncate_oldest(overflow);
    }

    inner.revision += 1;
    true
  }

  /// Document position right after the last line, used to pin the viewport
  /// to the newest record.
  pub(crate) fn end_position(&self) -> Option<Position> {
    let inner = self.inner.borrow();
    if inner.line_count == 0 {
      return None;
    }
    Some(inner.text.offset_to_position(inner.text.len()))
  }
}

impl Inner {
  /// Drop the oldest `count` lines, keeping marker rows consistent.
  fn truncate_oldest(&mut self, count: u64) {
    let count = count.min(self.line_count);
    if count == 0 {
      return;
    }
    let cut = self.text.line_start_offset(count as usize);
    self.text.remove(0..cut);
    self.line_count -= count;
    self.markers = self
      .markers
      .iter()
      .filter_map(|marker| {
        if marker.row < count {
          None
        } else {
          let mut marker = *marker;
          marker.row -= count;
          marker.end_row = marker.end_row.saturating_sub(count).max(marker.row);
          Some(marker)
        }
      })
      .collect();
  }
}

/// The [`EditorBackend`] half of a [`LogConsole`], handed to the editor
/// state.
pub(crate) struct LogConsoleBackend {
  console: LogConsole,
}

impl LogConsoleBackend {
  pub(crate) fn new(console: LogConsole) -> Self {
    Self { console }
  }
}

impl woocraft::EditorActionSink for LogConsoleBackend {}

impl woocraft::EditorContextMenuProvider for LogConsoleBackend {}

impl woocraft::EditorHighlighterProvider for LogConsoleBackend {
  fn create_highlighter(&self) -> Option<Box<dyn EditorHighlighter>> {
    Some(Box::new(TracingConsoleHighlighter::default()))
  }
}

impl EditorBackend for LogConsoleBackend {
  fn revision(&self) -> u64 {
    self.console.inner.borrow().revision
  }

  fn snapshot(&self) -> Arc<dyn EditorSnapshot> {
    let inner = self.console.inner.borrow();
    Arc::new(RopeEditorSnapshot::new(inner.revision, inner.text.clone()))
  }

  fn scrollbar_markers(&self) -> Vec<ScrollbarMarker> {
    self.console.inner.borrow().markers.clone()
  }
}

/// A parsed and cached console line.
#[derive(Debug)]
struct LineHighlight {
  byte_range: Range<u64>,
  spans: Vec<(Range<u64>, SpanKind)>,
}

/// Syntax highlighter rendering log records in the `tracing_subscriber`
/// console style.
#[derive(Default)]
struct TracingConsoleHighlighter {
  lines: Vec<LineHighlight>,
}

impl TracingConsoleHighlighter {
  fn style(kind: SpanKind, theme: &HighlightTheme) -> Option<HighlightStyle> {
    let syntax = &theme.style.syntax;
    match kind {
      SpanKind::Timestamp | SpanKind::Target => syntax.comment.map(HighlightStyle::from),
      SpanKind::Level(level) => {
        let theme_style = match level {
          Level::ERROR => syntax.string_special,
          Level::WARN => syntax.number,
          Level::INFO => syntax.string,
          Level::DEBUG => syntax.function,
          Level::TRACE => syntax.keyword,
        };
        theme_style.map(|style| {
          let mut style = HighlightStyle::from(style);
          style.font_weight = Some(FontWeight::BOLD);
          style
        })
      }
    }
  }
}

impl EditorHighlighter for TracingConsoleHighlighter {
  fn sync(&mut self, snapshot: &dyn EditorSnapshot, _change: Option<&woocraft::EditorTextChange>) {
    self.lines = (0..snapshot.line_count())
      .filter_map(|row| {
        let line = snapshot.line(row)?;
        let text = line.text.trim_end_matches('\n');
        let spans = parse_line(text)
          .into_iter()
          .map(|(range, kind)| {
            (
              line.byte_range.start + range.start..line.byte_range.start + range.end,
              kind,
            )
          })
          .collect::<Vec<_>>();
        Some(LineHighlight {
          byte_range: line.byte_range,
          spans,
        })
      })
      .collect();
  }

  fn highlight_range(
    &self, _snapshot: &dyn EditorSnapshot, range: Range<u64>, theme: &HighlightTheme,
  ) -> Vec<(Range<u64>, HighlightStyle)> {
    let first = self
      .lines
      .partition_point(|line| line.byte_range.end <= range.start);

    let mut highlights = Vec::new();
    for line in &self.lines[first..] {
      if line.byte_range.start >= range.end {
        break;
      }
      for (span, kind) in &line.spans {
        if span.start < range.end
          && span.end > range.start
          && let Some(style) = Self::style(*kind, theme)
        {
          highlights.push((span.clone(), style));
        }
      }
    }
    highlights
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_line_styles_timestamp_level_target() {
    let spans =
      parse_line("00:01.335 DEBUG craftoria_workbench::app: applying the default dock layout");
    assert_eq!(
      spans,
      vec![
        (0..9, SpanKind::Timestamp),
        (10..15, SpanKind::Level(Level::DEBUG)),
        (15..41, SpanKind::Target),
      ]
    );
  }

  #[test]
  fn parse_line_handles_padded_short_levels() {
    let spans = parse_line("00:00.000  INFO craftoria: starting");
    assert_eq!(spans[1], (11..15, SpanKind::Level(Level::INFO)));
    assert_eq!(spans[2], (15..26, SpanKind::Target));
  }

  #[test]
  fn parse_line_rejects_non_console_lines() {
    assert!(parse_line("a continuation line of a message").is_empty());
    assert!(parse_line("").is_empty());
  }

  #[test]
  fn console_caps_at_max_lines() {
    let console = LogConsole::default();
    let line = LogLine {
      elapsed: Duration::ZERO,
      level: Level::INFO,
      target: "craftoria".to_string(),
      message: "hello".to_string(),
    };
    let lines = vec![line; MAX_LINES as usize + 10];
    console.append(&lines);

    let inner = console.inner.borrow();
    assert_eq!(inner.line_count, MAX_LINES);
    assert_eq!(inner.text.lines_len(), MAX_LINES as usize + 1);
  }

  #[test]
  fn truncation_shifts_marker_rows() {
    let console = LogConsole::default();
    let error = LogLine {
      elapsed: Duration::ZERO,
      level: Level::ERROR,
      target: "craftoria".to_string(),
      message: "boom".to_string(),
    };
    console.append(&[error]);
    let markers = console.inner.borrow().markers.clone();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].row, 0);

    console.inner.borrow_mut().truncate_oldest(1);
    assert!(console.inner.borrow().markers.is_empty());
  }
}
