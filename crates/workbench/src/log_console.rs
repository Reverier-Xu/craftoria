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
//! style. Colors resolve through the active syntax theme so light/dark
//! switching keeps working.
//!
//! The highlighter contract requires the returned spans to tile the requested
//! byte range exactly, with no gaps and no overlaps: the editor turns them
//! into sequential text runs by length alone, so everything that is not a
//! styled field (message text, separators, newlines, unparsed lines) is
//! emitted as an identity [`gpui::HighlightStyle`] span that keeps the
//! editor's base font and foreground color.

use std::{
  ops::Range,
  sync::{Arc, Mutex},
  time::Duration,
};

use tracing::Level;
use woocraft::{
  EditorBackend, EditorBackendCapabilities, EditorHighlighter, EditorSnapshot, HighlightTheme,
  Rope, RopeEditorSnapshot, RopeExt as _, ScrollbarMarker,
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
  inner: Arc<Mutex<Inner>>,
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

    let mut inner = self.lock();

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

  fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
    self
      .inner
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    self.console.lock().revision
  }

  fn capabilities(&self) -> EditorBackendCapabilities {
    EditorBackendCapabilities {
      editable: false,
      custom_line_numbers: false,
      custom_highlighter: true,
    }
  }

  fn snapshot(&self) -> Arc<dyn EditorSnapshot> {
    let inner = self.console.lock();
    Arc::new(RopeEditorSnapshot::new(inner.revision, inner.text.clone()))
  }

  fn scrollbar_markers(&self) -> Vec<ScrollbarMarker> {
    self.console.lock().markers.clone()
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
    // The editor consumes these spans as an ordered run list, honoring only
    // their lengths: absolute offsets are dropped and the runs are laid over
    // the visible window back to back. The output therefore has to tile
    // `range` exactly — a single gap shifts every following run onto the
    // wrong bytes, and once the styled bytes run out the rest of the window
    // falls back to the platform's default font and color. Gaps are filled
    // with `HighlightStyle::default()`, an identity overlay that keeps the
    // editor's base text style (default font, theme foreground).
    let first = self
      .lines
      .partition_point(|line| line.byte_range.end <= range.start);

    let mut highlights = Vec::new();
    let mut cursor = range.start;
    for line in &self.lines[first..] {
      if line.byte_range.start >= range.end {
        break;
      }
      for (span, kind) in &line.spans {
        let start = span.start.max(range.start);
        let end = span.end.min(range.end);
        if start >= end {
          continue;
        }
        let Some(style) = Self::style(*kind, theme) else {
          continue;
        };
        if cursor < start {
          highlights.push((cursor..start, HighlightStyle::default()));
        }
        highlights.push((start..end, style));
        cursor = end;
      }
    }

    if cursor < range.end {
      highlights.push((cursor..range.end, HighlightStyle::default()));
    }
    highlights
  }
}

#[cfg(test)]
mod tests {
  use woocraft::{HighlightThemeStyle, SyntaxColors, ThemeMode, ThemeTokens};

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

  /// Build a theme with every syntax token populated, as the active woocraft
  /// theme would.
  fn test_theme() -> HighlightTheme {
    HighlightTheme {
      name: "test".to_string(),
      appearance: ThemeMode::Dark,
      style: HighlightThemeStyle {
        syntax: SyntaxColors::from_tokens(&ThemeTokens::default()),
        ..HighlightThemeStyle::default()
      },
    }
  }

  fn synced_highlighter(text: &str) -> TracingConsoleHighlighter {
    let snapshot = RopeEditorSnapshot::new(1, Rope::from(text));
    let mut highlighter = TracingConsoleHighlighter::default();
    EditorHighlighter::sync(&mut highlighter, &snapshot, None);
    highlighter
  }

  #[test]
  fn highlight_range_tiles_the_whole_range() {
    let first_line = "00:01.335 DEBUG craftoria::app: first\n";
    let text = "00:01.335 DEBUG craftoria::app: first\n00:02.000  INFO craftoria: second\n";
    let highlighter = synced_highlighter(text);
    let range = 0..text.len() as u64;
    let highlights = highlighter.highlight_range(
      &RopeEditorSnapshot::new(1, Rope::from(text)),
      range.clone(),
      &test_theme(),
    );

    // The spans must tile the requested range exactly: they start at
    // `range.start`, end at `range.end`, and leave no gap in between.
    assert_eq!(highlights.first().map(|(r, _)| r.start), Some(range.start));
    assert_eq!(highlights.last().map(|(r, _)| r.end), Some(range.end));
    for pair in highlights.windows(2) {
      assert_eq!(pair[0].0.end, pair[1].0.start, "spans must be contiguous");
    }

    let by_start = |start: u64| {
      highlights
        .iter()
        .find(|(r, _)| r.start == start)
        .map(|(_, s)| *s)
    };

    // Styled fields keep their exact byte offsets on line 1 …
    let timestamp = by_start(0).expect("timestamp span on line 1");
    assert!(timestamp.color.is_some());
    let level = by_start(10).expect("level span on line 1");
    assert!(level.color.is_some());
    assert_eq!(level.font_weight, Some(FontWeight::BOLD));
    let target = by_start(15).expect("target span on line 1");
    assert!(target.color.is_some());

    // … while the separator and the message render with the identity
    // style, so the editor's base font and foreground color apply to them.
    assert_eq!(by_start(9), Some(HighlightStyle::default()));
    assert_eq!(by_start(31), Some(HighlightStyle::default()));

    // Line 2 is styled the same way — the bug was every line after the
    // first losing its runs and falling back to the platform default font.
    let second = first_line.len() as u64;
    let level2 = by_start(second + 11).expect("level span on line 2");
    assert!(level2.color.is_some());
    assert_eq!(level2.font_weight, Some(FontWeight::BOLD));
    assert_eq!(
      by_start(second + 26),
      Some(HighlightStyle::default()),
      "message on line 2 must keep the identity style"
    );
  }

  #[test]
  fn highlight_range_clips_to_the_requested_range() {
    let first_line = "00:01.335 DEBUG craftoria::app: first\n";
    let text = "00:01.335 DEBUG craftoria::app: first\n00:02.000  INFO craftoria: second\n";
    let snapshot = RopeEditorSnapshot::new(1, Rope::from(text));
    let highlighter = synced_highlighter(text);

    // A window that starts inside line 1's target field and ends inside
    // line 2's target field must still be tiled from `range.start` to
    // `range.end`, with the clipped styled spans keeping their colors.
    let range = 25u64..(first_line.len() as u64 + 18);
    let highlights = highlighter.highlight_range(&snapshot, range.clone(), &test_theme());
    assert_eq!(highlights.first().map(|(r, _)| r.start), Some(range.start));
    assert_eq!(highlights.last().map(|(r, _)| r.end), Some(range.end));
    for pair in highlights.windows(2) {
      assert_eq!(pair[0].0.end, pair[1].0.start, "spans must be contiguous");
    }

    let by_start = |start: u64| {
      highlights
        .iter()
        .find(|(r, _)| r.start == start)
        .map(|(_, s)| *s)
    };
    let second = first_line.len() as u64;

    // Clipped head: line 1's target field is styled from `range.start` on.
    let clipped_target = by_start(range.start).expect("clipped target span");
    assert!(clipped_target.color.is_some());
    // Timestamp and level of line 2 land at their absolute offsets.
    assert!(
      by_start(second)
        .expect("timestamp span on line 2")
        .color
        .is_some()
    );
    let level2 = by_start(second + 11).expect("level span on line 2");
    assert_eq!(level2.font_weight, Some(FontWeight::BOLD));
    // Clipped tail: line 2's target field runs up to `range.end`.
    let tail = highlights.last().cloned();
    assert!(
      tail
        .as_ref()
        .is_some_and(|(_, style)| style.color.is_some())
    );
    assert_eq!(tail.as_ref().map(|(r, _)| r.end), Some(range.end));
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

    let inner = console.lock();
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
    let markers = console.lock().markers.clone();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].row, 0);

    console.lock().truncate_oldest(1);
    assert!(console.lock().markers.is_empty());
  }
}
