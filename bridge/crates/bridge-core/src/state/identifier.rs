use std::{fmt, str::FromStr, sync::Arc};

/// Kind of identifier rejected by the state subsystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateIdentifierKind {
    /// Session identifier.
    Session,
    /// Device identifier.
    Device,
    /// Browser connector identifier.
    Connector,
}

impl fmt::Display for StateIdentifierKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Session => "session",
            Self::Device => "device",
            Self::Connector => "connector",
        })
    }
}

/// Structured identifier validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateIdentifierError {
    kind: StateIdentifierKind,
    value: String,
}

impl StateIdentifierError {
    pub(super) fn empty(kind: StateIdentifierKind, value: String) -> Self {
        Self { kind, value }
    }

    /// Returns the rejected identifier kind.
    #[must_use]
    pub const fn kind(&self) -> StateIdentifierKind {
        self.kind
    }

    /// Returns the rejected identifier value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for StateIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} identifier must not be empty, got {:?}",
            self.kind, self.value
        )
    }
}

impl std::error::Error for StateIdentifierError {}

macro_rules! define_identifier {
    ($name:ident, $kind:expr, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Arc<str>);

        impl $name {
            #[doc = "Creates a validated identifier.\n\n# Errors\n\nReturns [`StateIdentifierError`] when `value` is empty."]
            pub fn new(value: impl Into<String>) -> Result<Self, StateIdentifierError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(StateIdentifierError::empty($kind, value));
                }
                Ok(Self(Arc::from(value)))
            }

            /// Returns the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = StateIdentifierError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = StateIdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

define_identifier!(SessionId, StateIdentifierKind::Session, "Validated session identifier.");
define_identifier!(DeviceId, StateIdentifierKind::Device, "Validated device identifier.");
define_identifier!(
    ConnectorId,
    StateIdentifierKind::Connector,
    "Validated browser connector identifier."
);

/// Runtime owner of a canonical capability set.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CapabilityOwner {
    /// Capabilities exposed by the Bridge itself.
    Bridge,
    /// Capabilities associated with a session.
    Session(SessionId),
    /// Capabilities associated with a device.
    Device(DeviceId),
    /// Capabilities associated with a browser connector.
    Connector(ConnectorId),
}

impl fmt::Display for CapabilityOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge => formatter.write_str("bridge"),
            Self::Session(identifier) => write!(formatter, "session:{identifier}"),
            Self::Device(identifier) => write!(formatter, "device:{identifier}"),
            Self::Connector(identifier) => write!(formatter, "connector:{identifier}"),
        }
    }
}
