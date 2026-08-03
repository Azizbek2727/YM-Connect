use std::sync::Arc;

use crate::{
    BridgeStateDraft, BridgeStateStore, ConnectionId, SessionId, StateError, StateUpdate,
    TransportCapabilities, TransportConnectionSnapshot, TransportEndpoint, TransportError,
    TransportResult, TransportRevision, TransportState, TransportTimestamp,
};

use super::TransportConnectionParts;

/// Command that creates a connection record in the `Created` lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateTransportConnection {
    connection_id: ConnectionId,
    endpoint: TransportEndpoint,
    capabilities: TransportCapabilities,
    created_at: TransportTimestamp,
}

impl CreateTransportConnection {
    /// Creates a connection-creation command.
    #[must_use]
    pub const fn new(
        connection_id: ConnectionId,
        endpoint: TransportEndpoint,
        capabilities: TransportCapabilities,
        created_at: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            endpoint,
            capabilities,
            created_at,
        }
    }
}

/// Command that performs one finite-state-machine transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionTransportConnection {
    connection_id: ConnectionId,
    expected_revision: TransportRevision,
    requested_state: TransportState,
    timestamp: TransportTimestamp,
}

impl TransitionTransportConnection {
    /// Creates a lifecycle-transition command.
    #[must_use]
    pub const fn new(
        connection_id: ConnectionId,
        expected_revision: TransportRevision,
        requested_state: TransportState,
        timestamp: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            expected_revision,
            requested_state,
            timestamp,
        }
    }
}

/// Command that binds an authenticated connection to an existing session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindTransportSession {
    connection_id: ConnectionId,
    expected_revision: TransportRevision,
    session_id: SessionId,
    timestamp: TransportTimestamp,
}

impl BindTransportSession {
    /// Creates a session-binding command.
    #[must_use]
    pub const fn new(
        connection_id: ConnectionId,
        expected_revision: TransportRevision,
        session_id: SessionId,
        timestamp: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            expected_revision,
            session_id,
            timestamp,
        }
    }
}

/// Command that removes a connection's session binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnbindTransportSession {
    connection_id: ConnectionId,
    expected_revision: TransportRevision,
    timestamp: TransportTimestamp,
}

impl UnbindTransportSession {
    /// Creates a session-unbinding command.
    #[must_use]
    pub const fn new(
        connection_id: ConnectionId,
        expected_revision: TransportRevision,
        timestamp: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            expected_revision,
            timestamp,
        }
    }
}

/// Command that advances a connection toward the `Closed` terminal state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseTransportConnection {
    connection_id: ConnectionId,
    expected_revision: TransportRevision,
    timestamp: TransportTimestamp,
}

impl CloseTransportConnection {
    /// Creates a connection-close command.
    #[must_use]
    pub const fn new(
        connection_id: ConnectionId,
        expected_revision: TransportRevision,
        timestamp: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            expected_revision,
            timestamp,
        }
    }
}

/// Result of a successful Transport Manager mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportMutation {
    connection: Arc<TransportConnectionSnapshot>,
    state_update: StateUpdate,
}

impl TransportMutation {
    fn new(connection: Arc<TransportConnectionSnapshot>, state_update: StateUpdate) -> Self {
        Self {
            connection,
            state_update,
        }
    }

    /// Returns the resulting immutable connection record.
    #[must_use]
    pub fn connection(&self) -> &TransportConnectionSnapshot {
        self.connection.as_ref()
    }

    /// Returns the committed Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.state_update
    }
}

/// Runtime-independent transport lifecycle orchestrator backed by Bridge State transactions.
#[derive(Clone, Debug)]
pub struct TransportManager {
    state: BridgeStateStore,
}

impl TransportManager {
    /// Creates a Transport Manager over an existing Bridge State store.
    #[must_use]
    pub fn new(state: BridgeStateStore) -> Self {
        Self { state }
    }

    /// Returns a cloneable handle to the authoritative Bridge State store.
    #[must_use]
    pub fn state_store(&self) -> BridgeStateStore {
        self.state.clone()
    }

    /// Creates a connection record.
    ///
    /// # Errors
    ///
    /// Returns a duplicate-connection or Bridge State error.
    pub fn create_connection(
        &self,
        command: CreateTransportConnection,
    ) -> TransportResult<TransportMutation> {
        let result_id = command.connection_id.clone();
        let update = self
            .state
            .update_with::<TransportError>(move |draft| {
                if draft.connections().contains_key(&command.connection_id) {
                    return Err(TransportError::DuplicateConnection {
                        connection_id: command.connection_id,
                    });
                }

                let transport_id = command.endpoint.transport_id().clone();
                let connection = TransportConnectionSnapshot::from_parts(
                    TransportConnectionParts {
                        connection_id: command.connection_id.clone(),
                        transport_id,
                        endpoint: command.endpoint,
                        capabilities: command.capabilities,
                        state: TransportState::Created,
                        session_id: None,
                        revision: TransportRevision::INITIAL,
                        created_at: command.created_at,
                        updated_at: command.created_at,
                    },
                );
                let _ = draft
                    .connections_mut()
                    .insert(connection)
                    .map_err(StateError::from)?;
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }

    /// Performs one explicit lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, revision, timestamp, lifecycle, or Bridge State error.
    pub fn transition_connection(
        &self,
        command: TransitionTransportConnection,
    ) -> TransportResult<TransportMutation> {
        self.transition(
            command.connection_id,
            command.expected_revision,
            command.requested_state,
            command.timestamp,
        )
    }

    /// Binds an authenticated connection to an existing session.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, revision, timestamp, lifecycle, binding, session, or state
    /// error.
    pub fn bind_session(
        &self,
        command: BindTransportSession,
    ) -> TransportResult<TransportMutation> {
        let result_id = command.connection_id.clone();
        let update = self
            .state
            .update_with::<TransportError>(move |draft| {
                let current = current_connection(draft, &command.connection_id)?;
                validate_current(
                    &current,
                    command.expected_revision,
                    command.timestamp,
                )?;
                if current.state() != TransportState::Authenticated {
                    return Err(TransportError::BindingRequiresAuthenticated {
                        connection_id: command.connection_id,
                        state: current.state(),
                    });
                }
                if !draft.sessions().contains_key(&command.session_id) {
                    return Err(TransportError::MissingSession {
                        session_id: command.session_id,
                    });
                }
                if let Some(session_id) = current.session_id() {
                    return Err(TransportError::SessionAlreadyBound {
                        connection_id: command.connection_id,
                        session_id: session_id.clone(),
                    });
                }

                let mut parts = current.to_parts();
                parts.session_id = Some(command.session_id);
                parts.updated_at = command.timestamp;
                parts.revision = next_revision(&current)?;
                let next = TransportConnectionSnapshot::from_parts(parts);
                let _ = draft
                    .connections_mut()
                    .replace(next)
                    .map_err(StateError::from)?;
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }

    /// Removes a connection's session binding.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, revision, timestamp, terminal, binding, or state error.
    pub fn unbind_session(
        &self,
        command: UnbindTransportSession,
    ) -> TransportResult<TransportMutation> {
        let result_id = command.connection_id.clone();
        let update = self
            .state
            .update_with::<TransportError>(move |draft| {
                let current = current_connection(draft, &command.connection_id)?;
                validate_current(
                    &current,
                    command.expected_revision,
                    command.timestamp,
                )?;
                if current.session_id().is_none() {
                    return Err(TransportError::SessionNotBound {
                        connection_id: command.connection_id,
                    });
                }

                let mut parts = current.to_parts();
                parts.session_id = None;
                parts.updated_at = command.timestamp;
                parts.revision = next_revision(&current)?;
                let next = TransportConnectionSnapshot::from_parts(parts);
                let _ = draft
                    .connections_mut()
                    .replace(next)
                    .map_err(StateError::from)?;
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }

    /// Advances a connection to `Closing`, or from `Closing` to terminal `Closed`.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, revision, timestamp, lifecycle, or state error.
    pub fn close_connection(
        &self,
        command: CloseTransportConnection,
    ) -> TransportResult<TransportMutation> {
        let current = self
            .lookup_connection(&command.connection_id)?
            .ok_or_else(|| TransportError::ConnectionNotFound {
                connection_id: command.connection_id.clone(),
            })?;
        let requested = if current.state() == TransportState::Closing {
            TransportState::Closed
        } else {
            TransportState::Closing
        };
        self.transition(
            command.connection_id,
            command.expected_revision,
            requested,
            command.timestamp,
        )
    }

    /// Looks up one immutable connection record.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn lookup_connection(
        &self,
        connection_id: &ConnectionId,
    ) -> TransportResult<Option<Arc<TransportConnectionSnapshot>>> {
        Ok(self
            .state
            .snapshot()?
            .connections()
            .get_shared(connection_id))
    }

    /// Lists immutable connection records in deterministic identifier order.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn list_connections(&self) -> TransportResult<Vec<Arc<TransportConnectionSnapshot>>> {
        let snapshot = self.state.snapshot()?;
        Ok(snapshot
            .connections()
            .keys()
            .filter_map(|connection_id| snapshot.connections().get_shared(connection_id))
            .collect())
    }

    /// Returns whether a connection identifier is registered.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn connection_exists(&self, connection_id: &ConnectionId) -> TransportResult<bool> {
        Ok(self
            .state
            .snapshot()?
            .connections()
            .contains_key(connection_id))
    }

    fn transition(
        &self,
        connection_id: ConnectionId,
        expected_revision: TransportRevision,
        requested_state: TransportState,
        timestamp: TransportTimestamp,
    ) -> TransportResult<TransportMutation> {
        let result_id = connection_id.clone();
        let update = self
            .state
            .update_with::<TransportError>(move |draft| {
                let current = current_connection(draft, &connection_id)?;
                validate_current(&current, expected_revision, timestamp)?;
                validate_transition(&current, requested_state)?;

                let mut parts = current.to_parts();
                parts.state = requested_state;
                parts.updated_at = timestamp;
                parts.revision = next_revision(&current)?;
                let next = TransportConnectionSnapshot::from_parts(parts);
                let _ = draft
                    .connections_mut()
                    .replace(next)
                    .map_err(StateError::from)?;
                Ok(())
            })?;

        mutation_from_update(&result_id, update)
    }
}

fn mutation_from_update(
    connection_id: &ConnectionId,
    update: StateUpdate,
) -> TransportResult<TransportMutation> {
    let connection = update
        .snapshot()
        .connections()
        .get_shared(connection_id)
        .ok_or_else(|| {
            TransportError::state_invariant(format!(
                "committed transport connection {connection_id} is absent from its resulting snapshot"
            ))
        })?;
    Ok(TransportMutation::new(connection, update))
}

fn current_connection(
    draft: &BridgeStateDraft,
    connection_id: &ConnectionId,
) -> TransportResult<Arc<TransportConnectionSnapshot>> {
    draft
        .connections()
        .get_shared(connection_id)
        .ok_or_else(|| TransportError::ConnectionNotFound {
            connection_id: connection_id.clone(),
        })
}

fn validate_current(
    connection: &TransportConnectionSnapshot,
    expected_revision: TransportRevision,
    timestamp: TransportTimestamp,
) -> TransportResult<()> {
    if connection.revision() != expected_revision {
        return Err(TransportError::StaleRevision {
            connection_id: connection.connection_id().clone(),
            expected: expected_revision,
            actual: connection.revision(),
        });
    }
    if connection.state().is_terminal() {
        return Err(TransportError::TerminalConnection {
            connection_id: connection.connection_id().clone(),
            state: connection.state(),
        });
    }
    if timestamp < connection.updated_at() {
        return Err(TransportError::TimestampRegression {
            connection_id: connection.connection_id().clone(),
            previous: connection.updated_at(),
            requested: timestamp,
        });
    }
    Ok(())
}

fn validate_transition(
    connection: &TransportConnectionSnapshot,
    requested: TransportState,
) -> TransportResult<()> {
    if !connection.state().can_transition_to(requested) {
        return Err(TransportError::InvalidTransition {
            connection_id: connection.connection_id().clone(),
            previous: connection.state(),
            requested,
        });
    }
    Ok(())
}

fn next_revision(connection: &TransportConnectionSnapshot) -> TransportResult<TransportRevision> {
    connection
        .revision()
        .checked_next()
        .ok_or_else(|| TransportError::RevisionExhausted {
            connection_id: connection.connection_id().clone(),
        })
}
