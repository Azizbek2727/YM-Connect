//! Runtime-independent primitives for the YM Connect desktop Bridge.
//!
//! This crate owns configuration, runtime state, session lifecycle orchestration, transport
//! contracts, pairing and trust orchestration, logging contracts, dependency injection, and the
//! application lifecycle. Platform, concrete transport, key persistence, and asynchronous-runtime
//! integrations belong in executable or adapter crates so the core remains browser-,
//! protocol-encoding-, and runtime-agnostic.

mod application;
mod config;
mod error;
mod logging;
mod pairing;
mod session;
mod shutdown;
mod state;
mod transport;

pub use application::{BridgeApplication, BridgeDependencies};
pub use config::{
    BridgeConfig, BridgeConfigLayer, BridgeConfigLoader, ConfigError, ConfigErrorKind,
    ConfigField, ConfigSource, DEFAULT_LOG_LEVEL, DEFAULT_RUNTIME_WORKER_THREADS, LOG_LEVEL_ENV,
    LoggingConfig, RUNTIME_WORKER_THREADS_ENV, RuntimeConfig, RuntimeWorkerThreads,
};
pub use error::BridgeError;
pub use logging::{LogLevel, LogRecord, Logger, StderrLogger};
pub use pairing::{
    BridgeId, BridgeIdentity, ChallengeId, CreatePairingChallenge, CreatePairingSession,
    EstablishPairingTrust, PairingAlgorithmSuite, PairingCapabilities, PairingChallenge,
    PairingConfirmationTag, PairingCryptoProvider, PairingEphemeralSecret,
    PairingEphemeralSecretSource, PairingError, PairingEvent, PairingEventKind, PairingId,
    PairingManager, PairingModelError, PairingMutation, PairingNonce, PairingPolicy,
    PairingPublicKey, PairingRequest, PairingResponse, PairingResult, PairingRevision,
    PairingSession, PairingState, PairingTimestamp, ReceivePairingResponse, RevokeTrustedPeer,
    RustCryptoPairingProvider, TransitionPairing, TrustDecision, TrustMetadata, TrustMetadataKey,
    TrustMetadataValue, TrustMutation, TrustStore, TrustedPeer, VerifyPairingIdentity,
};
pub use session::{
    BridgeSession, CloseSession, CreateSession, DEFAULT_SESSION_INACTIVITY_TIMEOUT_MS,
    ExpiredSessions, RemoveExpiredSessions, RestoreSession, ResumeSession, SessionCapabilityList,
    SessionDuration, SessionLifecycleState, SessionManager, SessionManagerError, SessionMetadata,
    SessionMetadataKey, SessionMetadataValue, SessionModelError, SessionMutation, SessionPolicy,
    SessionRevision, SessionStateTransition, SessionTimestamp, SuspendSession, UpdateSession,
};
pub use shutdown::{ShutdownError, ShutdownFuture, ShutdownSignal};
pub use state::{
    BridgeLifecycleState, BridgeStateChange, BridgeStateDraft, BridgeStateEvent,
    BridgeStateSnapshot, BridgeStateStore, BridgeStateSubscription, CapabilityOwner,
    CapabilityRegistration, CapabilityRegistry, ConnectionId, ConnectionRegistry, ConnectorId,
    ConnectorRegistry, DeviceId, DeviceRegistry, NotificationSummary, PairingSessionRegistry,
    RegistryDelta, RegistryFailure, RegistryKind, RegistryMutation, RegistryOperation,
    RegistryStateError, SessionId, SessionRegistry, StateError, StateIdentifierError,
    StateIdentifierKind, StateLock, StateReceiveError, StateRegistry, StateRegistryValue,
    StateRevision, StateUpdate, SubscriptionId, TransportId, TrustedPeerRegistry,
};
pub use transport::{
    BindTransportSession, CloseTransportConnection, CreateTransportConnection,
    TransitionTransportConnection, TransportCapabilities, TransportConnection,
    TransportConnectionSnapshot, TransportEndpoint, TransportEndpointAddress,
    TransportEndpointRole, TransportError, TransportEvent, TransportEventKind, TransportFactory,
    TransportFeature, TransportFuture, TransportManager, TransportMessageEnvelope,
    TransportModelError, TransportMutation, TransportOperation, TransportResult,
    TransportRevision, TransportState, TransportStatistics, TransportTimestamp,
    UnbindTransportSession,
};
