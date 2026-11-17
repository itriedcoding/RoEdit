//! Logging: a `log::Log` implementation that keeps the last N entries for the
//! Console panel. Errors and warnings are also counted for the diagnostics
//! panel.

use log::{Level, Metadata, Record};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct LogEntry {
    /// Seconds since the Unix epoch (UTC).
    pub timestamp: f64,
    pub level: Level,
    pub target: String,
    pub message: String,
}

impl LogEntry {
    pub fn time_string(&self) -> String {
        format_clock(self.timestamp)
    }

    pub fn formatted(&self) -> String {
        format!(
            "[{}] {:<5} {}: {}",
            self.time_string(),
            self.level.as_str(),
            self.target,
            self.message
        )
    }
}

/// Formats a Unix timestamp as `HH:MM:SS` in UTC without pulling in a date
/// library.
pub fn format_clock(ts: f64) -> String {
    let secs = ts.max(0.0) as u64;
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{h:02}:{m:02}:{s:02}")
}

#[derive(Debug, Default)]
struct Inner {
    entries: VecDeque<LogEntry>,
    capacity: usize,
    errors: u64,
    warnings: u64,
    dropped: u64,
}

#[derive(Debug)]
pub struct LogStore {
    inner: Mutex<Inner>,
}

impl LogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                entries: VecDeque::with_capacity(capacity.min(4096)),
                capacity: capacity.max(32),
                ..Default::default()
            }),
        }
    }

    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut inner) = self.inner.lock() {
            match entry.level {
                Level::Error => inner.errors += 1,
                Level::Warn => inner.warnings += 1,
                _ => {}
            }
            if inner.entries.len() >= inner.capacity {
                inner.entries.pop_front();
                inner.dropped += 1;
            }
            inner.entries.push_back(entry);
        }
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.inner
            .lock()
            .map(|i| i.entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        self.inner
            .lock()
            .map(|i| {
                i.entries
                    .iter()
                    .rev()
                    .take(count)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut i) = self.inner.lock() {
            i.entries.clear();
        }
    }

    pub fn counts(&self) -> (u64, u64, u64) {
        self.inner
            .lock()
            .map(|i| (i.errors, i.warnings, i.dropped))
            .unwrap_or((0, 0, 0))
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|i| i.entries.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_text(&self) -> String {
        self.entries()
            .iter()
            .map(|e| e.formatted())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl log::Log for LogStore {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        self.push(LogEntry {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
            level: record.level(),
            target: record.target().to_string(),
            message: record.args().to_string(),
        });
    }

    fn flush(&self) {}
}

static GLOBAL: OnceLock<Arc<LogStore>> = OnceLock::new();

/// Installs the global logger (idempotent) and returns the store.
pub fn init(capacity: usize) -> Arc<LogStore> {
    if let Some(existing) = GLOBAL.get() {
        return existing.clone();
    }
    let store = Arc::new(LogStore::new(capacity));
    let logger: Box<dyn log::Log> = Box::new(StoreHandle(store.clone()));
    let _ = log::set_logger(Box::leak(logger));
    log::set_max_level(log::LevelFilter::Info);
    let _ = GLOBAL.set(store.clone());
    store
}

/// Global store accessor; initialises a default store when needed.
pub fn global() -> Arc<LogStore> {
    GLOBAL.get().cloned().unwrap_or_else(|| init(2000))
}

struct StoreHandle(Arc<LogStore>);

impl log::Log for StoreHandle {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.0.enabled(metadata)
    }
    fn log(&self, record: &Record) {
        self.0.log(record)
    }
    fn flush(&self) {
        self.0.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_format() {
        assert_eq!(format_clock(0.0), "00:00:00");
        assert_eq!(format_clock(3661.0), "01:01:01");
        assert_eq!(format_clock(86399.0), "23:59:59");
    }

    #[test]
    fn ring_buffer_drops_oldest() {
        let store = LogStore::new(32);
        for i in 0..100 {
            store.push(LogEntry {
                timestamp: i as f64,
                level: Level::Info,
                target: "test".into(),
                message: format!("msg {i}"),
            });
        }
        assert_eq!(store.len(), 32);
        let entries = store.entries();
        assert!(entries.last().unwrap().message.contains("99"));
        let (errors, warnings, dropped) = store.counts();
        assert_eq!((errors, warnings, dropped), (0, 0, 68));
    }

    #[test]
    fn counts_errors_and_warnings() {
        let store = LogStore::new(64);
        for level in [Level::Error, Level::Warn, Level::Info] {
            store.push(LogEntry {
                timestamp: 0.0,
                level,
                target: "t".into(),
                message: "m".into(),
            });
        }
        assert_eq!(store.counts(), (1, 1, 0));
        assert!(store.as_text().contains("ERROR"));
        assert_eq!(store.recent(1).len(), 1);
    }
}
