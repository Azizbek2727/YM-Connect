use std::{error::Error, fmt, future::Future, pin::Pin};

/// Boxed future returned by an injected shutdown source.
pub type ShutdownFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), ShutdownError>> + Send + 'a>>;

/// Runtime-independent source of a graceful shutdown request.
pub trait ShutdownSignal: fmt::Debug + Send + Sync {
    /// Waits until shutdown is requested or signal observation fails.
    fn wait(&self) -> ShutdownFuture<'_>;
}

/// Failure produced by a shutdown-signal implementation.
#[derive(Debug)]
pub struct ShutdownError {
    context: &'static str,
    source: Box<dyn Error + Send + Sync>,
}

impl ShutdownError {
    /// Creates a shutdown failure with stable context and its original source.
    #[must_use]
    pub fn new(context: &'static str, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            context,
            source: Box::new(source),
        }
    }
}

impl fmt::Display for ShutdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.context, self.source)
    }
}

impl Error for ShutdownError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
