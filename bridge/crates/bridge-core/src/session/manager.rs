use std::sync::Arc;

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::{
    BridgeSession, BridgeStateDraft, BridgeStateStore, ConnectorId, DeviceId, SessionCapabilityList,
    SessionDuration, SessionId, SessionLifecycleState, SessionManagerError, SessionMetadata,
    SessionRevision, SessionStateTransition, SessionTimestamp, StateError, StateUpdate,
};

use super::SessionRecordParts;

/// Default inactivity timeout used by applications that choose the documented policy value.
pub const DEFAULT_SESSION_INACTIVITY_TIMEOUT_MS: u64 = 30 * 60 * 1_000;

/// Immutable Session Manager policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPolicy {
    inactivity_timeout: SessionDuration,
    enforce_unique_live_association: bool,
}

impl SessionPolicy {
    /// Creates a session policy.
    #[must_use]
    pub const fn new(inactivity_timeout: SessionDuration) -> Self {
        Self {
            inactivity_timeout,
            enforce_unique_live_association: true,
        }
    }

    /// Enables or disables the one-live-session-per-device/connector rule.
    #[must_use]
    pub const fn with_unique_live_association(mut self, enabled: bool) -> Self {
        self.enforce_unique_live_association = enabled;
        self
    }

    /// Returns the inactivity timeout.
    #[must_use]
    pub const fn inactivity_timeout(self) -> SessionDuration {
        self.inactivity_timeout
    }

    /// Returns whether live device/connector associations must be unique.
    #[must_use]
    pub const fn enforces_unique_live_association(self) -> bool {
        self.enforce_unique_live_association
    }
}

/// Command that creates a new session in the `Created` lifecycle state.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateSession {
    session_id: SessionId,
    device_id: DeviceId,
    connector_id: ConnectorId,
    capabilities: CapabilitySet,
    protocol_version: ProtocolVersion,
    created_at: SessionTimestamp,
    metadata: SessionMetadata,
}

impl CreateSession {
    /// Creates a session-creation command.
    #[must_use]
    pub fn new(
        session_id: SessionId,
        device_id: DeviceId,
        connector_id: ConnectorId,
        capabilities: CapabilitySet,
        protocol_version: ProtocolVersion,
        created_at: SessionTimestamp,
    ) -> Self {
        Self {
            session_id,
            device_id,
            connector_id,
            capabilities,
            protocol_version,
            created_at,
            metadata: SessionMetadata::new(),
        }
    }

    /// Replaces the initial metadata container.
    #[must_use]
    pub fn with_metadata(mut self, metadata: SessionMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Command that restores a persisted session record.
#[derive(Clone, Debug, PartialEq)]
pub struct RestoreSession {
    session_id: SessionId,
    device_id: DeviceId,
    connector_id: ConnectorId,
    capabilities: CapabilitySet,
    protocol_version: ProtocolVersion,
    lifecycle: SessionLifecycleState,
    revision: SessionRevision,
    created_at: SessionTimestamp,
    last_activity_at: SessionTimestamp,
    restored_at: SessionTimestamp,
    metadata: SessionMetadata,
}

impl RestoreSession {
    /// Creates a session-restoration command from the session's original creation input.
    #[must_use]
    pub fn new(
        original: CreateSession,
        lifecycle: SessionLifecycleState,
        revision: SessionRevision,
        last_activity_at: SessionTimestamp,
        restored_at: SessionTimestamp,
    ) -> Self {
        Self {
            session_id: original.session_id,
            device_id: original.device_id,
            connector_id: original.connector_id,
            capabilities: original.capabilities,
            protocol_version: original.protocol_version,
            lifecycle,
            revision,
            created_at: original.created_at,
            last_activity_at,
            restored_at,
            metadata: original.metadata,
        }
    }

    /// Replaces the restored metadata container.
    #[must_use]
    pub fn with_metadata(mut self, metadata: SessionMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Command that updates mutable session data and optionally performs one lifecycle transition.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateSession {
    session_id: SessionId,
    expected_revision: SessionRevision,
    timestamp: SessionTimestamp,
    lifecycle: Option<SessionLifecycleState>,
    capabilities: Option<CapabilitySet>,
    protocol_version: Option<ProtocolVersion>,
    metadata: Option<SessionMetadata>,
}

impl UpdateSession {
    /// Creates a session-update command.
    #[must_use]
    pub const fn new(
        session_id: SessionId,
        expected_revision: SessionRevision,
        timestamp: SessionTimestamp,
    ) -> Self {
        Self {
            session_id,
            expected_revision,
            timestamp,
            lifecycle: None,
            capabilities: None,
            protocol_version: None,
            metadata: None,
        }
    }

    /// Requests one finite-state-machine transition.
    #[must_use]
    pub const fn with_lifecycle(mut self, lifecycle: SessionLifecycleState) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    /// Replaces the negotiated capability set.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: CapabilitySet) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Replaces the negotiated protocol version.
    #[must_use]
    pub fn with_protocol_version(mut self, protocol_version: ProtocolVersion) -> Self {
        self.protocol_version = Some(protocol_version);
        self
    }

    /// Replaces the metadata container.
    #[must_use]
    pub fn with_metadata(mut self, metadata: SessionMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

macro_rules! define_transition_command {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name {
            session_id: SessionId,
            expected_revision: SessionRevision,
            timestamp: SessionTimestamp,
        }

        impl $name {
            /// Creates a lifecycle command.
            #[must_use]
            pub const fn new(
                session_id: SessionId,
                expected_revision: SessionRevision,
                timestamp: SessionTimestamp,
            ) -> Self {
                Self {
                    session_id,
                    expected_revision,
                    timestamp,
                }
            }
        }
    };
}

define_transition_command!(
    SuspendSession,
    "Command that transitions an active session to suspended."
);
define_transition_command!(
    ResumeSession,
    "Command that transitions a suspended session to active."
);
define_transition_command!(
    CloseSession,
    "Command that advances a session toward the closed terminal state."
);

/// Command that removes every session expired at one deterministic observation timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveExpiredSessions {
    observed_at: SessionTimestamp,
}

impl RemoveExpiredSessions {
    /// Creates an expiration command.
    #[must_use]
    pub const fn new(observed_at: SessionTimestamp) -> Self {
        Self { observed_at }
    }
}

/// Result of a successful create, restore, update, suspend, resume, or close operation.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionMutation {
    session: Arc<BridgeSession>,
    state_update: StateUpdate,
}

impl SessionMutation {
    fn new(session: Arc<BridgeSession>, state_update: StateUpdate) -> Self {
        Self {
            session,
            state_update,
        }
    }

    /// Returns the resulting immutable session record.
    #[must_use]
    pub fn session(&self) -> &BridgeSession {
        self.session.as_ref()
    }

    /// Returns the committed Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.state_update
    }
}

/// Result of one deterministic expiration sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpiredSessions {
    removed: Arc<[SessionId]>,
    state_update: StateUpdate,
}

impl ExpiredSessions {
    fn new(removed: Vec<SessionId>, state_update: StateUpdate) -> Self {
        Self {
            removed: removed.into(),
            state_update,
        }
    }

    /// Returns removed identifiers in deterministic order.
    #[must_use]
    pub fn removed(&self) -> &[SessionId] {
        &self.removed
    }

    /// Returns the committed Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.state_update
    }
}

/// Runtime-independent lifecycle orchestrator backed exclusively by Bridge State transactions.
#[derive(Clone, Debug)]
pub struct SessionManager {
    state: BridgeStateStore,
    policy: SessionPolicy,
}

impl SessionManager {
    /// Creates a Session Manager over an existing Bridge State store.
    #[must_use]
    pub fn new(state: BridgeStateStore, policy: SessionPolicy) -> Self {
        Self { state, policy }
    }

    /// Returns a cloneable handle to the authoritative Bridge State store.
    #[must_use]
    pub fn state_store(&self) -> BridgeStateStore {
        self.state.clone()
    }

    /// Returns the immutable orchestration policy.
    #[must_use]
    pub const fn policy(&self) -> SessionPolicy {
        self.policy
    }

    /// Creates a new session.
    ///
    /// # Errors
    ///
    /// Returns a structured error when identifiers conflict, referenced state is missing,
    /// negotiated values are invalid, or Bridge State cannot commit the transaction.
    pub fn create_session(
        &self,
        command: CreateSession,
    ) -> Result<SessionMutation, SessionManagerError> {
        let capabilities = normalize_capabilities(command.capabilities)?;
        validate_protocol_version(&command.protocol_version)?;
        let result_id = command.session_id.clone();
        let policy = self.policy;

        let update = self
            .state
            .update_with::<SessionManagerError>(move |draft| {
                validate_associations(draft, &command.device_id, &command.connector_id)?;
                validate_new_session(
                    draft,
                    policy,
                    &command.session_id,
                    &command.device_id,
                    &command.connector_id,
                    SessionLifecycleState::Created,
                )?;

                let session = BridgeSession::from_parts(SessionRecordParts {
                    session_id: command.session_id.clone(),
                    created_at: command.created_at,
                    last_activity_at: command.created_at,
                    lifecycle: SessionLifecycleState::Created,
                    device_id: command.device_id,
                    connector_id: command.connector_id,
                    capabilities,
                    protocol_version: command.protocol_version,
                    revision: SessionRevision::INITIAL,
                    metadata: command.metadata,
                });
                let _ = draft
                    .sessions_mut()
                    .insert(session)
                    .map_err(StateError::from)?;
                draft.record_session_transition(SessionStateTransition::new(
                    command.session_id,
                    None,
                    Some(SessionLifecycleState::Created),
                    SessionRevision::INITIAL,
                    command.created_at,
                ));
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }

    /// Restores a persisted session.
    ///
    /// # Errors
    ///
    /// Returns a structured error when timestamps, associations, negotiated values, expiration,
    /// uniqueness, or the Bridge State transaction are invalid.
    pub fn restore_session(
        &self,
        command: RestoreSession,
    ) -> Result<SessionMutation, SessionManagerError> {
        validate_protocol_version(&command.protocol_version)?;
        validate_restore_timestamps(&command)?;
        validate_elapsed(
            &command.session_id,
            command.last_activity_at,
            command.restored_at,
            self.policy.inactivity_timeout,
        )?;
        let capabilities = normalize_capabilities(command.capabilities)?;
        let result_id = command.session_id.clone();
        let policy = self.policy;

        let update = self
            .state
            .update_with::<SessionManagerError>(move |draft| {
                validate_associations(draft, &command.device_id, &command.connector_id)?;
                validate_new_session(
                    draft,
                    policy,
                    &command.session_id,
                    &command.device_id,
                    &command.connector_id,
                    command.lifecycle,
                )?;

                let session = BridgeSession::from_parts(SessionRecordParts {
                    session_id: command.session_id.clone(),
                    created_at: command.created_at,
                    last_activity_at: command.last_activity_at,
                    lifecycle: command.lifecycle,
                    device_id: command.device_id,
                    connector_id: command.connector_id,
                    capabilities,
                    protocol_version: command.protocol_version,
                    revision: command.revision,
                    metadata: command.metadata,
                });
                let _ = draft
                    .sessions_mut()
                    .insert(session)
                    .map_err(StateError::from)?;
                draft.record_session_transition(SessionStateTransition::new(
                    command.session_id,
                    None,
                    Some(command.lifecycle),
                    command.revision,
                    command.restored_at,
                ));
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }

    /// Updates mutable session fields and optionally performs one lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns a structured error for missing, stale, expired, terminal, invalid-transition,
    /// invalid-negotiation, or Bridge State failures.
    pub fn update_session(
        &self,
        mut command: UpdateSession,
    ) -> Result<SessionMutation, SessionManagerError> {
        if let Some(capabilities) = command.capabilities.take() {
            command.capabilities = Some(normalize_capabilities(capabilities)?);
        }
        if let Some(protocol_version) = command.protocol_version.as_ref() {
            validate_protocol_version(protocol_version)?;
        }
        let result_id = command.session_id.clone();
        let timeout = self.policy.inactivity_timeout;

        let update = self
            .state
            .update_with::<SessionManagerError>(move |draft| {
                let current = current_session(draft, &command.session_id)?;
                validate_current(
                    &current,
                    command.expected_revision,
                    command.timestamp,
                    timeout,
                )?;

                let mut parts = current.to_parts();
                let previous_lifecycle = parts.lifecycle;
                if let Some(requested) = command.lifecycle {
                    validate_transition(&current, requested)?;
                    parts.lifecycle = requested;
                }
                if let Some(capabilities) = command.capabilities {
                    parts.capabilities = capabilities;
                }
                if let Some(protocol_version) = command.protocol_version {
                    parts.protocol_version = protocol_version;
                }
                if let Some(metadata) = command.metadata {
                    parts.metadata = metadata;
                }

                let changed = parts.lifecycle != current.lifecycle()
                    || &parts.capabilities != current.capabilities()
                    || &parts.protocol_version != current.protocol_version()
                    || &parts.metadata != current.metadata()
                    || command.timestamp != current.last_activity_at();
                if !changed {
                    return Ok(());
                }

                parts.last_activity_at = command.timestamp;
                parts.revision = next_revision(&current)?;
                let next = BridgeSession::from_parts(parts);
                let next_revision = next.revision();
                let next_lifecycle = next.lifecycle();
                let _ = draft
                    .sessions_mut()
                    .replace(next)
                    .map_err(StateError::from)?;

                if previous_lifecycle != next_lifecycle {
                    draft.record_session_transition(SessionStateTransition::new(
                        command.session_id,
                        Some(previous_lifecycle),
                        Some(next_lifecycle),
                        next_revision,
                        command.timestamp,
                    ));
                }
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }

    /// Suspends an active session.
    ///
    /// # Errors
    ///
    /// Returns a structured lifecycle, revision, expiration, lookup, or state error.
    pub fn suspend_session(
        &self,
        command: SuspendSession,
    ) -> Result<SessionMutation, SessionManagerError> {
        self.transition_session(
            command.session_id,
            command.expected_revision,
            command.timestamp,
            TransitionIntent::Suspend,
        )
    }

    /// Resumes a suspended session.
    ///
    /// # Errors
    ///
    /// Returns a structured lifecycle, revision, expiration, lookup, or state error.
    pub fn resume_session(
        &self,
        command: ResumeSession,
    ) -> Result<SessionMutation, SessionManagerError> {
        self.transition_session(
            command.session_id,
            command.expected_revision,
            command.timestamp,
            TransitionIntent::Resume,
        )
    }

    /// Advances a session to `Closing`, or from `Closing` to terminal `Closed`.
    ///
    /// # Errors
    ///
    /// Returns a structured lifecycle, revision, expiration, lookup, or state error.
    pub fn close_session(
        &self,
        command: CloseSession,
    ) -> Result<SessionMutation, SessionManagerError> {
        self.transition_session(
            command.session_id,
            command.expected_revision,
            command.timestamp,
            TransitionIntent::Close,
        )
    }

    /// Removes every session expired at `command.observed_at` in one atomic transaction.
    ///
    /// # Errors
    ///
    /// Returns a timestamp or Bridge State error. No session is removed when validation fails.
    pub fn remove_expired_sessions(
        &self,
        command: RemoveExpiredSessions,
    ) -> Result<ExpiredSessions, SessionManagerError> {
        let timeout = self.policy.inactivity_timeout;
        let mut removed = Vec::new();

        let update = self
            .state
            .update_with::<SessionManagerError>(|draft| {
                let mut candidates = Vec::new();
                for (session_id, session) in draft.sessions().iter() {
                    let Some(elapsed) = command
                        .observed_at
                        .checked_duration_since(session.last_activity_at())
                    else {
                        return Err(SessionManagerError::TimestampRegression {
                            session_id: session_id.clone(),
                            previous: session.last_activity_at(),
                            requested: command.observed_at,
                        });
                    };
                    if elapsed >= timeout.as_millis() {
                        candidates.push((
                            session_id.clone(),
                            session.lifecycle(),
                            session.revision(),
                        ));
                    }
                }

                for (session_id, lifecycle, revision) in candidates {
                    let _ = draft
                        .sessions_mut()
                        .remove(&session_id)
                        .map_err(StateError::from)?;
                    draft.record_session_transition(SessionStateTransition::new(
                        session_id.clone(),
                        Some(lifecycle),
                        None,
                        revision,
                        command.observed_at,
                    ));
                    removed.push(session_id);
                }
                Ok(())
            })?;

        Ok(ExpiredSessions::new(removed, update))
    }

    /// Looks up one immutable session record.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn lookup_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<Arc<BridgeSession>>, SessionManagerError> {
        Ok(self.state.snapshot()?.sessions().get_shared(session_id))
    }

    /// Lists immutable session records in deterministic identifier order.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn list_sessions(&self) -> Result<Vec<Arc<BridgeSession>>, SessionManagerError> {
        let snapshot = self.state.snapshot()?;
        Ok(snapshot
            .sessions()
            .keys()
            .filter_map(|session_id| snapshot.sessions().get_shared(session_id))
            .collect())
    }

    /// Returns whether a session identifier is registered.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn session_exists(&self, session_id: &SessionId) -> Result<bool, SessionManagerError> {
        Ok(self.state.snapshot()?.sessions().contains_key(session_id))
    }

    fn transition_session(
        &self,
        session_id: SessionId,
        expected_revision: SessionRevision,
        timestamp: SessionTimestamp,
        intent: TransitionIntent,
    ) -> Result<SessionMutation, SessionManagerError> {
        let result_id = session_id.clone();
        let timeout = self.policy.inactivity_timeout;
        let update = self
            .state
            .update_with::<SessionManagerError>(move |draft| {
                let current = current_session(draft, &session_id)?;
                validate_current(&current, expected_revision, timestamp, timeout)?;
                let requested = match intent {
                    TransitionIntent::Suspend => SessionLifecycleState::Suspended,
                    TransitionIntent::Resume => SessionLifecycleState::Active,
                    TransitionIntent::Close
                        if current.lifecycle() == SessionLifecycleState::Closing =>
                    {
                        SessionLifecycleState::Closed
                    }
                    TransitionIntent::Close => SessionLifecycleState::Closing,
                };
                validate_transition(&current, requested)?;

                let previous = current.lifecycle();
                let mut parts = current.to_parts();
                parts.lifecycle = requested;
                parts.last_activity_at = timestamp;
                parts.revision = next_revision(&current)?;
                let next = BridgeSession::from_parts(parts);
                let revision = next.revision();
                let _ = draft
                    .sessions_mut()
                    .replace(next)
                    .map_err(StateError::from)?;
                draft.record_session_transition(SessionStateTransition::new(
                    session_id,
                    Some(previous),
                    Some(requested),
                    revision,
                    timestamp,
                ));
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }
}

#[derive(Clone, Copy)]
enum TransitionIntent {
    Suspend,
    Resume,
    Close,
}

fn mutation_from_update(
    session_id: &SessionId,
    update: StateUpdate,
) -> Result<SessionMutation, SessionManagerError> {
    let session = update
        .snapshot()
        .sessions()
        .get_shared(session_id)
        .ok_or_else(|| {
            SessionManagerError::state_invariant(format!(
                "committed session {session_id} is absent from its resulting snapshot"
            ))
        })?;
    Ok(SessionMutation::new(session, update))
}

fn current_session(
    draft: &BridgeStateDraft,
    session_id: &SessionId,
) -> Result<Arc<BridgeSession>, SessionManagerError> {
    draft
        .sessions()
        .get_shared(session_id)
        .ok_or_else(|| SessionManagerError::SessionNotFound {
            session_id: session_id.clone(),
        })
}

fn validate_associations(
    draft: &BridgeStateDraft,
    device_id: &DeviceId,
    connector_id: &ConnectorId,
) -> Result<(), SessionManagerError> {
    if !draft.devices().contains_key(device_id) {
        return Err(SessionManagerError::MissingDevice {
            device_id: device_id.clone(),
        });
    }
    if !draft.connectors().contains_key(connector_id) {
        return Err(SessionManagerError::MissingConnector {
            connector_id: connector_id.clone(),
        });
    }
    Ok(())
}

fn validate_new_session(
    draft: &BridgeStateDraft,
    policy: SessionPolicy,
    session_id: &SessionId,
    device_id: &DeviceId,
    connector_id: &ConnectorId,
    lifecycle: SessionLifecycleState,
) -> Result<(), SessionManagerError> {
    if draft.sessions().contains_key(session_id) {
        return Err(SessionManagerError::DuplicateSession {
            session_id: session_id.clone(),
        });
    }
    if !policy.enforce_unique_live_association || !lifecycle.is_live() {
        return Ok(());
    }
    if let Some((conflicting_id, _)) = draft.sessions().iter().find(|(_, session)| {
        session.lifecycle().is_live()
            && session.device_id() == device_id
            && session.connector_id() == connector_id
    }) {
        return Err(SessionManagerError::DuplicateLiveAssociation {
            session_id: session_id.clone(),
            conflicting_session_id: conflicting_id.clone(),
            device_id: device_id.clone(),
            connector_id: connector_id.clone(),
        });
    }
    Ok(())
}

fn validate_restore_timestamps(command: &RestoreSession) -> Result<(), SessionManagerError> {
    if command.created_at <= command.last_activity_at
        && command.last_activity_at <= command.restored_at
    {
        return Ok(());
    }
    Err(SessionManagerError::InvalidRestoreTimestamps {
        session_id: command.session_id.clone(),
        created_at: command.created_at,
        last_activity_at: command.last_activity_at,
        restored_at: command.restored_at,
    })
}

fn validate_current(
    session: &BridgeSession,
    expected_revision: SessionRevision,
    timestamp: SessionTimestamp,
    timeout: SessionDuration,
) -> Result<(), SessionManagerError> {
    if session.revision() != expected_revision {
        return Err(SessionManagerError::StaleRevision {
            session_id: session.session_id().clone(),
            expected: expected_revision,
            actual: session.revision(),
        });
    }
    if session.lifecycle().is_terminal() {
        return Err(SessionManagerError::TerminalSession {
            session_id: session.session_id().clone(),
            state: session.lifecycle(),
        });
    }
    validate_elapsed(
        session.session_id(),
        session.last_activity_at(),
        timestamp,
        timeout,
    )
}

fn validate_elapsed(
    session_id: &SessionId,
    previous: SessionTimestamp,
    requested: SessionTimestamp,
    timeout: SessionDuration,
) -> Result<(), SessionManagerError> {
    let Some(elapsed) = requested.checked_duration_since(previous) else {
        return Err(SessionManagerError::TimestampRegression {
            session_id: session_id.clone(),
            previous,
            requested,
        });
    };
    if elapsed >= timeout.as_millis() {
        return Err(SessionManagerError::ExpiredSession {
            session_id: session_id.clone(),
            last_activity_at: previous,
            observed_at: requested,
            timeout,
        });
    }
    Ok(())
}

fn validate_transition(
    session: &BridgeSession,
    requested: SessionLifecycleState,
) -> Result<(), SessionManagerError> {
    if session.lifecycle().is_terminal() {
        return Err(SessionManagerError::TerminalSession {
            session_id: session.session_id().clone(),
            state: session.lifecycle(),
        });
    }
    if !session.lifecycle().can_transition_to(requested) {
        return Err(SessionManagerError::InvalidTransition {
            session_id: session.session_id().clone(),
            previous: session.lifecycle(),
            requested,
        });
    }
    Ok(())
}

fn next_revision(session: &BridgeSession) -> Result<SessionRevision, SessionManagerError> {
    session
        .revision()
        .checked_next()
        .ok_or_else(|| SessionManagerError::RevisionExhausted {
            session_id: session.session_id().clone(),
        })
}

fn validate_protocol_version(version: &ProtocolVersion) -> Result<(), SessionManagerError> {
    if version.major == 0 {
        return Err(SessionManagerError::InvalidProtocolVersion {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
        });
    }
    Ok(())
}

fn normalize_capabilities(
    mut capabilities: CapabilitySet,
) -> Result<CapabilitySet, SessionManagerError> {
    validate_capability_list(
        &mut capabilities.supported,
        SessionCapabilityList::Supported,
    )?;
    validate_capability_list(&mut capabilities.required, SessionCapabilityList::Required)?;

    for required in &capabilities.required {
        if capabilities.supported.binary_search(required).is_err() {
            return Err(SessionManagerError::MissingRequiredCapability { value: *required });
        }
    }
    Ok(capabilities)
}

fn validate_capability_list(
    values: &mut Vec<i32>,
    list: SessionCapabilityList,
) -> Result<(), SessionManagerError> {
    for value in values.iter().copied() {
        let capability = Capability::try_from(value)
            .map_err(|_| SessionManagerError::InvalidCapability { list, value })?;
        if capability == Capability::CapabilityUnspecified {
            return Err(SessionManagerError::InvalidCapability { list, value });
        }
    }
    values.sort_unstable();
    if let Some(duplicate) = values
        .windows(2)
        .find_map(|window| (window[0] == window[1]).then_some(window[0]))
    {
        return Err(SessionManagerError::DuplicateCapability {
            list,
            value: duplicate,
        });
    }
    Ok(())
}
