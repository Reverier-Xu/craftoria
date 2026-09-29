//! In-memory log ring buffer fed by a `tracing` layer.
//!
//! The shell installs [`stream_layer`] next to the stderr formatter, so every
//! `tracing` event is mirrored into a process-global, bounded buffer. UI panels
//! (such as the bottom-dock log panel) poll the buffer with a cursor and
//! receive only the records they have not seen yet.

use std::{
  collections::VecDeque,
  fmt,
  sync::{Arc, Mutex, OnceLock},
  time::{Duration, Instant},
};

use tracing::{
  Event, Level, Subscriber,
  field::{Field, Visit},
};
use tracing_subscriber::layer::{Context, Layer};

/// Maximum number of retained log lines; the oldest lines are dropped first.
const CAPACITY: usize = 4096;

/// A single captured log record.
#[derive(Debug, Clone)]
pub struct LogLine {
  /// Time elapsed between process start and the record.
  pub elapsed: Duration,
  /// Severity of the record.
  pub level: Level,
  /// The target (module path) that emitted the record.
  pub target: String,
  /// The formatted message, with extra fields appended as `key=value`.
  pub message: String,
}

/// The result of draining new records from a [`LogBuffer`].
#[derive(Debug, Default)]
pub struct Drain {
  /// Records newer than the consumer cursor (or a full snapshot on `reset`).
  pub lines: Vec<LogLine>,
  /// The cursor to pass to the next [`LogBuffer::drain_since`] call.
  pub cursor: u64,
  /// `true` when the consumer fell behind and received a full snapshot
  /// instead of an incremental batch.
  pub reset: bool,
}

#[derive(Debug, Default)]
struct Inner {
  lines: VecDeque<LogLine>,
  /// Total number of records ever pushed; also the next cursor value.
  pushed: u64,
}

/// A bounded, thread-safe ring buffer of log records.
#[derive(Debug)]
pub struct LogBuffer {
  started: Instant,
  inner: Mutex<Inner>,
}

impl LogBuffer {
  fn push(&self, line: LogLine) {
    let mut inner = self
      .inner
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    if inner.lines.len() == CAPACITY {
      inner.lines.pop_front();
    }
    inner.lines.push_back(line);
    inner.pushed += 1;
  }

  /// Total number of records ever pushed into the buffer.
  pub fn total(&self) -> u64 {
    self
      .inner
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .pushed
  }

  /// Drain every record newer than `cursor`.
  ///
  /// When `cursor` points at records that were already evicted, a full
  /// snapshot of the retained records is returned with `reset` set, so the
  /// consumer can rebuild its view instead of skipping records silently.
  pub fn drain_since(&self, cursor: u64) -> Drain {
    let inner = self
      .inner
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cursor >= inner.pushed {
      return Drain {
        cursor,
        ..Drain::default()
      };
    }
    let available = inner.lines.len() as u64;
    let oldest = inner.pushed - available;
    if cursor < oldest {
      return Drain {
        lines: inner.lines.iter().cloned().collect(),
        cursor: inner.pushed,
        reset: true,
      };
    }
    let skip = (cursor - oldest) as usize;
    Drain {
      lines: inner.lines.iter().skip(skip).cloned().collect(),
      cursor: inner.pushed,
      reset: false,
    }
  }
}

/// The process-global log buffer shared by the tracing layer and the UI.
pub fn global() -> Arc<LogBuffer> {
  static GLOBAL: OnceLock<Arc<LogBuffer>> = OnceLock::new();
  GLOBAL
    .get_or_init(|| {
      Arc::new(LogBuffer {
        started: Instant::now(),
        inner: Mutex::new(Inner::default()),
      })
    })
    .clone()
}

/// A `tracing` layer that mirrors every event into the global [`LogBuffer`].
pub fn stream_layer() -> StreamLayer {
  StreamLayer { buffer: global() }
}

/// `tracing_subscriber::Layer` implementation returned by [`stream_layer`].
pub struct StreamLayer {
  buffer: Arc<LogBuffer>,
}

impl<S: Subscriber> Layer<S> for StreamLayer {
  fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
    let mut visitor = MessageVisitor::default();
    event.record(&mut visitor);
    self.buffer.push(LogLine {
      elapsed: self.buffer.started.elapsed(),
      level: *event.metadata().level(),
      target: event.metadata().target().to_string(),
      message: visitor.into_message(),
    });
  }
}

/// Extracts the `message` field and collects the remaining fields as
/// `key=value` pairs.
#[derive(Default)]
struct MessageVisitor {
  message: String,
  fields: Vec<String>,
}

impl MessageVisitor {
  fn into_message(self) -> String {
    if self.fields.is_empty() {
      self.message
    } else {
      format!("{} {}", self.message, self.fields.join(" "))
    }
  }
}

impl Visit for MessageVisitor {
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    let text = format!("{value:?}");
    if field.name() == "message" {
      self.message = text;
    } else {
      self.fields.push(format!("{}={text}", field.name()));
    }
  }

  fn record_str(&mut self, field: &Field, value: &str) {
    if field.name() == "message" {
      self.message = value.to_string();
    } else {
      self.fields.push(format!("{}={value}", field.name()));
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn buffer() -> LogBuffer {
    LogBuffer {
      started: Instant::now(),
      inner: Mutex::new(Inner::default()),
    }
  }

  fn line(message: &str) -> LogLine {
    LogLine {
      elapsed: Duration::ZERO,
      level: Level::INFO,
      target: "craftoria".to_string(),
      message: message.to_string(),
    }
  }

  #[test]
  fn drain_since_returns_only_new_records() {
    let buffer = buffer();
    buffer.push(line("one"));
    buffer.push(line("two"));

    let first = buffer.drain_since(0);
    assert_eq!(first.lines.len(), 2);
    assert_eq!(first.cursor, 2);
    assert!(!first.reset);

    buffer.push(line("three"));
    let second = buffer.drain_since(first.cursor);
    assert_eq!(second.lines.len(), 1);
    assert_eq!(second.lines[0].message, "three");
    assert_eq!(second.cursor, 3);
  }

  #[test]
  fn drain_since_is_empty_when_up_to_date() {
    let buffer = buffer();
    buffer.push(line("one"));
    let drain = buffer.drain_since(1);
    assert!(drain.lines.is_empty());
    assert!(!drain.reset);
  }

  #[test]
  fn drain_since_resets_when_the_consumer_fell_behind() {
    let buffer = buffer();
    for index in 0..CAPACITY + 8 {
      buffer.push(line(&format!("line {index}")));
    }

    let drain = buffer.drain_since(0);
    assert!(drain.reset);
    assert_eq!(drain.lines.len(), CAPACITY);
    assert_eq!(drain.lines[0].message, "line 8");
    assert_eq!(drain.cursor, (CAPACITY + 8) as u64);
  }

  #[test]
  fn capacity_drops_the_oldest_records() {
    let buffer = buffer();
    for index in 0..CAPACITY + 1 {
      buffer.push(line(&format!("line {index}")));
    }
    let drain = buffer.drain_since(1);
    assert!(!drain.reset);
    assert_eq!(drain.lines.len(), CAPACITY);
    assert_eq!(drain.lines[0].message, "line 1");
  }
}
