use std::{
    fmt,
    io::{self, Write},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

/// Severity and verbosity levels supported by the Bridge logger.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogLevel {
    /// Disables all log output.
    Off,
    /// Reports failures that prevent or interrupt correct operation.
    Error,
    /// Reports recoverable problems and degraded behavior.
    Warn,
    /// Reports normal lifecycle transitions.
    #[default]
    Info,
    /// Reports diagnostic state useful during development and support.
    Debug,
    /// Reports highly detailed execution diagnostics.
    Trace,
}

impl LogLevel {
    /// Returns the stable uppercase label used in structured records.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }

    const fn allows(self, record_level: Self) -> bool {
        !matches!(self, Self::Off) && record_level <= self
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("off") {
            Some(Self::Off)
        } else if value.eq_ignore_ascii_case("error") {
            Some(Self::Error)
        } else if value.eq_ignore_ascii_case("warn") || value.eq_ignore_ascii_case("warning") {
            Some(Self::Warn)
        } else if value.eq_ignore_ascii_case("info") {
            Some(Self::Info)
        } else if value.eq_ignore_ascii_case("debug") {
            Some(Self::Debug)
        } else if value.eq_ignore_ascii_case("trace") {
            Some(Self::Trace)
        } else {
            None
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A borrowed structured log event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogRecord<'a> {
    level: LogLevel,
    target: &'a str,
    message: &'a str,
}

impl<'a> LogRecord<'a> {
    /// Creates a structured log event.
    #[must_use]
    pub const fn new(level: LogLevel, target: &'a str, message: &'a str) -> Self {
        Self {
            level,
            target,
            message,
        }
    }

    /// Returns the event severity.
    #[must_use]
    pub const fn level(self) -> LogLevel {
        self.level
    }

    /// Returns the stable subsystem target.
    #[must_use]
    pub const fn target(self) -> &'a str {
        self.target
    }

    /// Returns the human-readable event message.
    #[must_use]
    pub const fn message(self) -> &'a str {
        self.message
    }
}

/// Logging boundary injected into the runtime-independent Bridge core.
pub trait Logger: fmt::Debug + Send + Sync {
    /// Emits a record when it passes the implementation's configured filter.
    fn log(&self, record: LogRecord<'_>);
}

/// Thread-safe structured logger that writes one record per line to standard error.
pub struct StderrLogger {
    filter: LogLevel,
    stderr: Mutex<io::Stderr>,
}

impl StderrLogger {
    /// Creates a standard-error logger with the supplied verbosity filter.
    #[must_use]
    pub fn new(filter: LogLevel) -> Self {
        Self {
            filter,
            stderr: Mutex::new(io::stderr()),
        }
    }
}

impl fmt::Debug for StderrLogger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StderrLogger")
            .field("filter", &self.filter)
            .finish_non_exhaustive()
    }
}

impl Logger for StderrLogger {
    fn log(&self, record: LogRecord<'_>) {
        if !self.filter.allows(record.level()) {
            return;
        }

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let line = format_record(timestamp_ms, record);
        let mut stderr = self
            .stderr
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = stderr.write_all(line.as_bytes());
        let _ = stderr.flush();
    }
}

fn format_record(timestamp_ms: u128, record: LogRecord<'_>) -> String {
    let target = escape_quoted(record.target());
    let message = escape_quoted(record.message());
    format!(
        "timestamp_unix_ms={timestamp_ms} level={} target=\"{target}\" message=\"{message}\"\n",
        record.level()
    )
}

fn escape_quoted(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

#[cfg(test)]
mod tests {
    use super::{LogLevel, LogRecord, format_record};

    #[test]
    fn structured_records_are_single_line_and_escaped() {
        let line = format_record(
            42,
            LogRecord::new(LogLevel::Warn, "bridge\"core", "first\nsecond"),
        );

        assert_eq!(
            line,
            "timestamp_unix_ms=42 level=WARN target=\"bridge\\\"core\" message=\"first\\nsecond\"\n"
        );
    }

    #[test]
    fn verbosity_filter_orders_severity_before_detail() {
        assert!(LogLevel::Info.allows(LogLevel::Error));
        assert!(LogLevel::Info.allows(LogLevel::Info));
        assert!(!LogLevel::Info.allows(LogLevel::Debug));
        assert!(!LogLevel::Off.allows(LogLevel::Error));
    }
}
