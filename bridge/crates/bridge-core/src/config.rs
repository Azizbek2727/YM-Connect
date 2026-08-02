use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt,
    num::{NonZeroUsize, ParseIntError},
};

use crate::LogLevel;

/// Environment variable controlling the minimum emitted log level.
pub const LOG_LEVEL_ENV: &str = "YM_CONNECT_LOG_LEVEL";

/// Environment variable controlling the Tokio worker-thread count.
pub const RUNTIME_WORKER_THREADS_ENV: &str = "YM_CONNECT_RUNTIME_WORKER_THREADS";

/// Validated lifecycle configuration for the Bridge skeleton.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConfig {
    log_level: LogLevel,
    runtime: RuntimeConfig,
}

impl BridgeConfig {
    /// Loads configuration from the process environment.
    ///
    /// Missing variables use production defaults. Set
    /// [`RUNTIME_WORKER_THREADS_ENV`] to `auto` to retain Tokio's automatic sizing.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a configured value is non-Unicode or invalid.
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(env::var_os)
    }

    /// Returns the configured minimum log level.
    #[must_use]
    pub const fn log_level(&self) -> LogLevel {
        self.log_level
    }

    /// Returns the asynchronous-runtime configuration.
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeConfig {
        &self.runtime
    }

    fn load_from(mut lookup: impl FnMut(&str) -> Option<OsString>) -> Result<Self, ConfigError> {
        let log_level = match read_unicode(&mut lookup, LOG_LEVEL_ENV)? {
            Some(value) => LogLevel::parse(&value).ok_or_else(|| ConfigError::InvalidLogLevel {
                variable: LOG_LEVEL_ENV,
                value,
            })?,
            None => LogLevel::Info,
        };

        let worker_threads = match read_unicode(&mut lookup, RUNTIME_WORKER_THREADS_ENV)? {
            Some(value) if value.eq_ignore_ascii_case("auto") => None,
            Some(value) => Some(parse_worker_threads(value)?),
            None => None,
        };

        Ok(Self {
            log_level,
            runtime: RuntimeConfig { worker_threads },
        })
    }
}

/// Validated configuration for the asynchronous runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    worker_threads: Option<NonZeroUsize>,
}

impl RuntimeConfig {
    /// Returns an explicit worker count, or `None` for automatic sizing.
    #[must_use]
    pub const fn worker_threads(&self) -> Option<NonZeroUsize> {
        self.worker_threads
    }
}

/// A configuration-loading failure.
#[derive(Debug)]
pub enum ConfigError {
    /// An environment variable was not valid Unicode.
    NonUnicode {
        /// Name of the invalid variable.
        variable: &'static str,
    },
    /// The requested log level was unsupported.
    InvalidLogLevel {
        /// Name of the invalid variable.
        variable: &'static str,
        /// Invalid configured value.
        value: String,
    },
    /// The requested runtime worker count was not a positive integer.
    InvalidWorkerThreads {
        /// Name of the invalid variable.
        variable: &'static str,
        /// Invalid configured value.
        value: String,
        /// Integer parsing failure, when parsing reached that stage.
        source: Option<ParseIntError>,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUnicode { variable } => {
                write!(formatter, "configuration variable {variable} is not valid Unicode")
            }
            Self::InvalidLogLevel { variable, value } => write!(
                formatter,
                "configuration variable {variable} has unsupported log level {value:?}"
            ),
            Self::InvalidWorkerThreads {
                variable, value, ..
            } => write!(
                formatter,
                "configuration variable {variable} must be 'auto' or a positive integer, got {value:?}"
            ),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidWorkerThreads {
                source: Some(source),
                ..
            } => Some(source),
            Self::NonUnicode { .. }
            | Self::InvalidLogLevel { .. }
            | Self::InvalidWorkerThreads { source: None, .. } => None,
        }
    }
}

fn read_unicode(
    lookup: &mut impl FnMut(&str) -> Option<OsString>,
    variable: &'static str,
) -> Result<Option<String>, ConfigError> {
    lookup(variable)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| ConfigError::NonUnicode { variable })
        })
        .transpose()
}

fn parse_worker_threads(value: String) -> Result<NonZeroUsize, ConfigError> {
    match value.parse::<usize>() {
        Ok(parsed) => NonZeroUsize::new(parsed).ok_or_else(|| ConfigError::InvalidWorkerThreads {
            variable: RUNTIME_WORKER_THREADS_ENV,
            value,
            source: None,
        }),
        Err(source) => Err(ConfigError::InvalidWorkerThreads {
            variable: RUNTIME_WORKER_THREADS_ENV,
            value,
            source: Some(source),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, ffi::OsString, num::NonZeroUsize};

    use super::{
        BridgeConfig, ConfigError, LOG_LEVEL_ENV, RUNTIME_WORKER_THREADS_ENV, RuntimeConfig,
    };
    use crate::LogLevel;

    fn load(values: &[(&str, &str)]) -> Result<BridgeConfig, ConfigError> {
        let values = values
            .iter()
            .map(|(key, value)| (*key, OsString::from(value)))
            .collect::<HashMap<_, _>>();
        BridgeConfig::load_from(|key| values.get(key).cloned())
    }

    #[test]
    fn defaults_are_safe_and_quiet() -> Result<(), ConfigError> {
        let config = load(&[])?;

        assert_eq!(config.log_level(), LogLevel::Info);
        assert_eq!(
            config.runtime(),
            &RuntimeConfig {
                worker_threads: None
            }
        );
        Ok(())
    }

    #[test]
    fn environment_overrides_lifecycle_settings() -> Result<(), ConfigError> {
        let config = load(&[
            (LOG_LEVEL_ENV, "debug"),
            (RUNTIME_WORKER_THREADS_ENV, "4"),
        ])?;

        assert_eq!(config.log_level(), LogLevel::Debug);
        assert_eq!(config.runtime().worker_threads(), NonZeroUsize::new(4));
        Ok(())
    }

    #[test]
    fn automatic_worker_sizing_is_explicitly_supported() -> Result<(), ConfigError> {
        let config = load(&[(RUNTIME_WORKER_THREADS_ENV, "AUTO")])?;

        assert_eq!(config.runtime().worker_threads(), None);
        Ok(())
    }

    #[test]
    fn zero_worker_threads_are_rejected() {
        assert!(matches!(
            load(&[(RUNTIME_WORKER_THREADS_ENV, "0")]),
            Err(ConfigError::InvalidWorkerThreads { source: None, .. })
        ));
    }

    #[test]
    fn unknown_log_levels_are_rejected() {
        assert!(matches!(
            load(&[(LOG_LEVEL_ENV, "verbose")]),
            Err(ConfigError::InvalidLogLevel { .. })
        ));
    }
}
