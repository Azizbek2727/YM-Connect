//! Strongly typed, runtime-independent Bridge configuration.
//!
//! Resolution starts with built-in defaults, applies programmatic layers in insertion order,
//! and applies process environment variables last unless environment loading is disabled.

use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt,
    num::{NonZeroUsize, ParseIntError},
};

use crate::LogLevel;

const LOG_LEVEL_EXPECTED: &str = "off, error, warn, info, debug, or trace";
const RUNTIME_WORKER_THREADS_EXPECTED: &str = "'auto' or a positive integer";

/// Environment variable controlling the minimum emitted log level.
pub const LOG_LEVEL_ENV: &str = "YM_CONNECT_LOG_LEVEL";

/// Environment variable controlling the asynchronous-runtime worker-thread count.
pub const RUNTIME_WORKER_THREADS_ENV: &str = "YM_CONNECT_RUNTIME_WORKER_THREADS";

/// Default minimum emitted log level.
pub const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Info;

/// Default asynchronous-runtime worker-thread policy.
pub const DEFAULT_RUNTIME_WORKER_THREADS: RuntimeWorkerThreads =
    RuntimeWorkerThreads::Automatic;

/// Fully resolved, validated, immutable Bridge configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BridgeConfig {
    logging: LoggingConfig,
    runtime: RuntimeConfig,
}

impl BridgeConfig {
    /// Loads built-in defaults followed by process environment overrides.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when an environment value cannot be decoded or validated.
    pub fn load() -> Result<Self, ConfigError> {
        BridgeConfigLoader::new().load()
    }

    /// Creates a loader for adding typed configuration layers before environment overrides.
    #[must_use]
    pub fn loader() -> BridgeConfigLoader {
        BridgeConfigLoader::new()
    }

    /// Returns immutable logging configuration.
    #[must_use]
    pub const fn logging(&self) -> &LoggingConfig {
        &self.logging
    }

    /// Returns immutable asynchronous-runtime configuration.
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeConfig {
        &self.runtime
    }
}

/// Immutable logging configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoggingConfig {
    level: LogLevel,
}

impl LoggingConfig {
    /// Returns the minimum emitted log level.
    #[must_use]
    pub const fn level(self) -> LogLevel {
        self.level
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: DEFAULT_LOG_LEVEL,
        }
    }
}

/// Immutable asynchronous-runtime configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    worker_threads: RuntimeWorkerThreads,
}

impl RuntimeConfig {
    /// Returns the configured worker-thread policy.
    #[must_use]
    pub const fn worker_threads(self) -> RuntimeWorkerThreads {
        self.worker_threads
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: DEFAULT_RUNTIME_WORKER_THREADS,
        }
    }
}

/// Worker-thread policy for the asynchronous runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeWorkerThreads {
    /// Lets the runtime select its platform-appropriate worker count.
    #[default]
    Automatic,
    /// Uses an explicit positive worker count.
    Fixed(NonZeroUsize),
}

impl RuntimeWorkerThreads {
    /// Creates a fixed worker-thread policy.
    #[must_use]
    pub const fn fixed(worker_threads: NonZeroUsize) -> Self {
        Self::Fixed(worker_threads)
    }

    /// Returns the explicit worker count, or `None` for automatic sizing.
    #[must_use]
    pub const fn as_fixed(self) -> Option<NonZeroUsize> {
        match self {
            Self::Automatic => None,
            Self::Fixed(worker_threads) => Some(worker_threads),
        }
    }
}

impl fmt::Display for RuntimeWorkerThreads {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Automatic => formatter.write_str("automatic"),
            Self::Fixed(worker_threads) => worker_threads.fmt(formatter),
        }
    }
}

/// A typed partial configuration layer.
///
/// Unset fields leave the value produced by lower-precedence layers unchanged.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BridgeConfigLayer {
    log_level: Option<LogLevel>,
    runtime_worker_threads: Option<RuntimeWorkerThreads>,
}

impl BridgeConfigLayer {
    /// Creates an empty layer that does not override any values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            log_level: None,
            runtime_worker_threads: None,
        }
    }

    /// Overrides the minimum emitted log level in this layer.
    #[must_use]
    pub const fn with_log_level(mut self, level: LogLevel) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Overrides the asynchronous-runtime worker-thread policy in this layer.
    #[must_use]
    pub const fn with_runtime_worker_threads(
        mut self,
        worker_threads: RuntimeWorkerThreads,
    ) -> Self {
        self.runtime_worker_threads = Some(worker_threads);
        self
    }

    fn apply_to(self, config: &mut BridgeConfig) {
        if let Some(level) = self.log_level {
            config.logging.level = level;
        }
        if let Some(worker_threads) = self.runtime_worker_threads {
            config.runtime.worker_threads = worker_threads;
        }
    }
}

/// Resolves immutable Bridge configuration from ordered layers.
///
/// Built-in defaults always form the lowest-precedence layer. Typed layers are applied in
/// insertion order, and process environment variables are applied last by default.
#[derive(Clone, Debug)]
pub struct BridgeConfigLoader {
    layers: Vec<BridgeConfigLayer>,
    include_environment: bool,
}

impl BridgeConfigLoader {
    /// Creates a loader using defaults followed by process environment overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a typed layer before process environment overrides.
    ///
    /// Later calls have higher precedence than earlier calls.
    #[must_use]
    pub fn with_layer(mut self, layer: BridgeConfigLayer) -> Self {
        self.layers.push(layer);
        self
    }

    /// Disables process environment loading.
    ///
    /// This is intended for deterministic embedding and tests. Built-in defaults and typed
    /// layers remain active.
    #[must_use]
    pub fn without_environment(mut self) -> Self {
        self.include_environment = false;
        self
    }

    /// Resolves and validates an immutable configuration snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when an enabled environment source contains an invalid value.
    pub fn load(self) -> Result<BridgeConfig, ConfigError> {
        self.load_with_environment(|variable| env::var_os(variable))
    }

    fn load_with_environment(
        self,
        mut lookup: impl FnMut(&'static str) -> Option<OsString>,
    ) -> Result<BridgeConfig, ConfigError> {
        let mut config = BridgeConfig::default();

        for layer in self.layers {
            layer.apply_to(&mut config);
        }

        if self.include_environment {
            environment_layer(&mut lookup)?.apply_to(&mut config);
        }

        Ok(config)
    }
}

impl Default for BridgeConfigLoader {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            include_environment: true,
        }
    }
}

/// Stable configuration field identifiers used by validation diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigField {
    /// Minimum emitted log level.
    LoggingLevel,
    /// Asynchronous-runtime worker-thread policy.
    RuntimeWorkerThreads,
}

impl ConfigField {
    /// Returns the stable dotted field name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoggingLevel => "logging.level",
            Self::RuntimeWorkerThreads => "runtime.worker_threads",
        }
    }
}

impl fmt::Display for ConfigField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Configuration source associated with a validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigSource {
    /// A process environment variable.
    EnvironmentVariable {
        /// Environment variable name.
        variable: &'static str,
    },
}

impl fmt::Display for ConfigSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnvironmentVariable { variable } => {
                write!(formatter, "environment variable {variable}")
            }
        }
    }
}

/// Structured reason a configuration value failed validation.
#[derive(Debug)]
pub enum ConfigErrorKind {
    /// The source value could not be represented as Unicode.
    NonUnicode,
    /// The source value was syntactically valid text but unsupported for the field.
    UnsupportedValue {
        /// Rejected value.
        value: String,
        /// Human-readable accepted-value contract.
        expected: &'static str,
    },
    /// The source value could not be parsed as an integer.
    InvalidInteger {
        /// Rejected value.
        value: String,
        /// Integer parser failure.
        source: ParseIntError,
    },
}

/// A structured configuration decoding or validation failure.
#[derive(Debug)]
pub struct ConfigError {
    field: ConfigField,
    config_source: ConfigSource,
    kind: ConfigErrorKind,
}

impl ConfigError {
    fn new(field: ConfigField, config_source: ConfigSource, kind: ConfigErrorKind) -> Self {
        Self {
            field,
            config_source,
            kind,
        }
    }

    /// Returns the field that failed validation.
    #[must_use]
    pub const fn field(&self) -> ConfigField {
        self.field
    }

    /// Returns the source that supplied the invalid value.
    #[must_use]
    pub const fn config_source(&self) -> ConfigSource {
        self.config_source
    }

    /// Returns the structured failure reason.
    #[must_use]
    pub const fn kind(&self) -> &ConfigErrorKind {
        &self.kind
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "configuration field {} from {} ",
            self.field, self.config_source
        )?;

        match &self.kind {
            ConfigErrorKind::NonUnicode => formatter.write_str("is not valid Unicode"),
            ConfigErrorKind::UnsupportedValue { value, expected } => {
                write!(formatter, "has unsupported value {value:?}; expected {expected}")
            }
            ConfigErrorKind::InvalidInteger { value, .. } => write!(
                formatter,
                "has invalid integer value {value:?}; expected {RUNTIME_WORKER_THREADS_EXPECTED}"
            ),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ConfigErrorKind::InvalidInteger { source, .. } => Some(source),
            ConfigErrorKind::NonUnicode | ConfigErrorKind::UnsupportedValue { .. } => None,
        }
    }
}

fn environment_layer(
    lookup: &mut impl FnMut(&'static str) -> Option<OsString>,
) -> Result<BridgeConfigLayer, ConfigError> {
    let mut layer = BridgeConfigLayer::new();

    if let Some(value) = read_unicode_environment_value(
        lookup,
        ConfigField::LoggingLevel,
        LOG_LEVEL_ENV,
    )? {
        let level = LogLevel::parse(&value).ok_or(ConfigError::new(
            ConfigField::LoggingLevel,
            ConfigSource::EnvironmentVariable {
                variable: LOG_LEVEL_ENV,
            },
            ConfigErrorKind::UnsupportedValue {
                value,
                expected: LOG_LEVEL_EXPECTED,
            },
        ))?;
        layer = layer.with_log_level(level);
    }

    if let Some(value) = read_unicode_environment_value(
        lookup,
        ConfigField::RuntimeWorkerThreads,
        RUNTIME_WORKER_THREADS_ENV,
    )? {
        layer = layer.with_runtime_worker_threads(parse_runtime_worker_threads(value)?);
    }

    Ok(layer)
}

fn read_unicode_environment_value(
    lookup: &mut impl FnMut(&'static str) -> Option<OsString>,
    field: ConfigField,
    variable: &'static str,
) -> Result<Option<String>, ConfigError> {
    lookup(variable)
        .map(|value| {
            value.into_string().map_err(|_| {
                ConfigError::new(
                    field,
                    ConfigSource::EnvironmentVariable { variable },
                    ConfigErrorKind::NonUnicode,
                )
            })
        })
        .transpose()
}

fn parse_runtime_worker_threads(value: String) -> Result<RuntimeWorkerThreads, ConfigError> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(RuntimeWorkerThreads::Automatic);
    }

    let parsed = match value.parse::<usize>() {
        Ok(parsed) => parsed,
        Err(source) => {
            return Err(ConfigError::new(
                ConfigField::RuntimeWorkerThreads,
                ConfigSource::EnvironmentVariable {
                    variable: RUNTIME_WORKER_THREADS_ENV,
                },
                ConfigErrorKind::InvalidInteger { value, source },
            ));
        }
    };

    NonZeroUsize::new(parsed)
        .map(RuntimeWorkerThreads::Fixed)
        .ok_or(ConfigError::new(
            ConfigField::RuntimeWorkerThreads,
            ConfigSource::EnvironmentVariable {
                variable: RUNTIME_WORKER_THREADS_ENV,
            },
            ConfigErrorKind::UnsupportedValue {
                value,
                expected: RUNTIME_WORKER_THREADS_EXPECTED,
            },
        ))
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::HashMap, error::Error, ffi::OsString, num::NonZeroUsize};

    use super::{
        BridgeConfig, BridgeConfigLayer, BridgeConfigLoader, ConfigError, ConfigErrorKind,
        ConfigField, ConfigSource, DEFAULT_LOG_LEVEL, DEFAULT_RUNTIME_WORKER_THREADS,
        LOG_LEVEL_ENV, RUNTIME_WORKER_THREADS_ENV, RuntimeWorkerThreads,
    };
    use crate::LogLevel;

    fn load(
        loader: BridgeConfigLoader,
        values: &[(&'static str, &str)],
    ) -> Result<BridgeConfig, ConfigError> {
        let values = values
            .iter()
            .map(|(key, value)| (*key, OsString::from(value)))
            .collect::<HashMap<_, _>>();
        loader.load_with_environment(|key| values.get(key).cloned())
    }

    #[test]
    fn defaults_are_complete_and_valid() -> Result<(), ConfigError> {
        let config = load(BridgeConfig::loader(), &[])?;

        assert_eq!(config.logging().level(), DEFAULT_LOG_LEVEL);
        assert_eq!(
            config.runtime().worker_threads(),
            DEFAULT_RUNTIME_WORKER_THREADS
        );
        Ok(())
    }

    #[test]
    fn later_typed_layers_override_earlier_layers() -> Result<(), ConfigError> {
        let fixed_workers = RuntimeWorkerThreads::fixed(NonZeroUsize::MIN);
        let config = load(
            BridgeConfig::loader()
                .without_environment()
                .with_layer(
                    BridgeConfigLayer::new()
                        .with_log_level(LogLevel::Debug)
                        .with_runtime_worker_threads(fixed_workers),
                )
                .with_layer(
                    BridgeConfigLayer::new()
                        .with_log_level(LogLevel::Trace)
                        .with_runtime_worker_threads(RuntimeWorkerThreads::Automatic),
                ),
            &[],
        )?;

        assert_eq!(config.logging().level(), LogLevel::Trace);
        assert_eq!(
            config.runtime().worker_threads(),
            RuntimeWorkerThreads::Automatic
        );
        Ok(())
    }

    #[test]
    fn environment_has_highest_precedence() -> Result<(), ConfigError> {
        let fixed_workers = RuntimeWorkerThreads::fixed(NonZeroUsize::MIN);
        let config = load(
            BridgeConfig::loader().with_layer(
                BridgeConfigLayer::new()
                    .with_log_level(LogLevel::Trace)
                    .with_runtime_worker_threads(fixed_workers),
            ),
            &[
                (LOG_LEVEL_ENV, "warn"),
                (RUNTIME_WORKER_THREADS_ENV, "2"),
            ],
        )?;

        assert_eq!(config.logging().level(), LogLevel::Warn);
        assert_eq!(
            config.runtime().worker_threads().as_fixed(),
            NonZeroUsize::new(2)
        );
        Ok(())
    }

    #[test]
    fn environment_loading_can_be_disabled() -> Result<(), ConfigError> {
        let environment_queried = Cell::new(false);
        let config = BridgeConfig::loader()
            .without_environment()
            .load_with_environment(|_| {
                environment_queried.set(true);
                None
            })?;

        assert!(!environment_queried.get());
        assert_eq!(config.logging().level(), DEFAULT_LOG_LEVEL);
        Ok(())
    }

    #[test]
    fn environment_values_are_case_insensitive() -> Result<(), ConfigError> {
        let config = load(
            BridgeConfig::loader(),
            &[
                (LOG_LEVEL_ENV, "WARNING"),
                (RUNTIME_WORKER_THREADS_ENV, "AUTO"),
            ],
        )?;

        assert_eq!(config.logging().level(), LogLevel::Warn);
        assert_eq!(
            config.runtime().worker_threads(),
            RuntimeWorkerThreads::Automatic
        );
        Ok(())
    }

    #[test]
    fn unsupported_log_level_has_structured_diagnostics() {
        assert!(matches!(
            load(BridgeConfig::loader(), &[(LOG_LEVEL_ENV, "verbose")]),
            Err(error)
                if error.field() == ConfigField::LoggingLevel
                    && error.config_source()
                        == (ConfigSource::EnvironmentVariable {
                            variable: LOG_LEVEL_ENV,
                        })
                    && matches!(error.kind(), ConfigErrorKind::UnsupportedValue { .. })
        ));
    }

    #[test]
    fn zero_worker_threads_are_rejected() {
        assert!(matches!(
            load(
                BridgeConfig::loader(),
                &[(RUNTIME_WORKER_THREADS_ENV, "0")],
            ),
            Err(error)
                if error.field() == ConfigField::RuntimeWorkerThreads
                    && matches!(error.kind(), ConfigErrorKind::UnsupportedValue { .. })
        ));
    }

    #[test]
    fn invalid_worker_integer_retains_parser_source() {
        assert!(matches!(
            load(
                BridgeConfig::loader(),
                &[(RUNTIME_WORKER_THREADS_ENV, "many")],
            ),
            Err(error)
                if error.field() == ConfigField::RuntimeWorkerThreads
                    && matches!(error.kind(), ConfigErrorKind::InvalidInteger { .. })
                    && Error::source(&error).is_some()
        ));
    }

    #[cfg(unix)]
    fn invalid_unicode_value() -> OsString {
        use std::os::unix::ffi::OsStringExt;

        OsString::from_vec(vec![0xff])
    }

    #[cfg(windows)]
    fn invalid_unicode_value() -> OsString {
        use std::os::windows::ffi::OsStringExt;

        OsString::from_wide(&[0xd800])
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn non_unicode_environment_value_is_rejected() {
        let values = HashMap::from([(LOG_LEVEL_ENV, invalid_unicode_value())]);

        assert!(matches!(
            BridgeConfig::loader().load_with_environment(|key| values.get(key).cloned()),
            Err(error)
                if error.field() == ConfigField::LoggingLevel
                    && matches!(error.kind(), ConfigErrorKind::NonUnicode)
        ));
    }
}
