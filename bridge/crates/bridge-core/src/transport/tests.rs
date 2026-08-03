use std::{
    error::Error,
    io,
    sync::{Arc, Barrier},
    task::{Context, Poll, Waker},
    thread,
};

use ym_connect_protocol::v1::{
    BrowserDescriptor, CapabilitySet, DeviceDescriptor, ProtocolVersion,
};

use crate::*;

use super::TransportConnectionParts;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn transport_id() -> TestResult<TransportId> {
    Ok(TransportId::new("test-transport")?)
}

fn connection_id(value: impl Into<String>) -> TestResult<ConnectionId> {
    Ok(ConnectionId::new(value.into())?)
}

fn timestamp(value: u64) -> TransportTimestamp {
    TransportTimestamp::from_unix_millis(value)
}

fn endpoint(address: impl Into<String>) -> TestResult<TransportEndpoint> {
    Ok(TransportEndpoint::new(
        transport_id()?,
        TransportEndpointAddress::new(address)?,
        TransportEndpointRole::Peer,
    ))
}

fn capabilities() -> TestResult<TransportCapabilities> {
    Ok(TransportCapabilities::new()
        .with_feature(TransportFeature::ReliableDelivery)
        .with_feature(TransportFeature::OrderedDelivery)
        .with_feature(TransportFeature::Bidirectional)
        .with_maximum_envelope_size(64 * 1024)?)
}

fn manager() -> TransportManager {
    TransportManager::new(BridgeStateStore::default())
}

fn create(
    manager: &TransportManager,
    identifier: impl Into<String>,
    at: u64,
) -> TestResult<TransportMutation> {
    let identifier = connection_id(identifier)?;
    Ok(manager.create_connection(CreateTransportConnection::new(
        identifier,
        endpoint("test://endpoint")?,
        capabilities()?,
        timestamp(at),
    ))?)
}

fn insert_connection(
    manager: &TransportManager,
    identifier: &str,
    state: TransportState,
    revision: u64,
    updated_at: u64,
) -> TestResult {
    let connection_id = connection_id(identifier)?;
    let endpoint = endpoint(format!("test://{identifier}"))?;
    let transport_id = endpoint.transport_id().clone();
    let capabilities = capabilities()?;
    manager.state_store().update(|draft| {
        let _ = draft.connections_mut().insert(
            TransportConnectionSnapshot::from_parts(TransportConnectionParts {
                connection_id,
                transport_id,
                endpoint,
                capabilities,
                state,
                session_id: None,
                revision: TransportRevision::new(revision),
                created_at: timestamp(1),
                updated_at: timestamp(updated_at),
            }),
        )?;
        Ok(())
    })?;
    Ok(())
}

fn add_session(store: &BridgeStateStore) -> TestResult<SessionId> {
    let session_id = SessionId::new("session-a")?;
    store.update(|draft| {
        let _ = draft.devices_mut().insert(DeviceDescriptor {
            device_id: "device-a".to_owned(),
            ..DeviceDescriptor::default()
        })?;
        let _ = draft.connectors_mut().insert(BrowserDescriptor {
            connector_id: "connector-a".to_owned(),
            ..BrowserDescriptor::default()
        })?;
        Ok(())
    })?;
    let sessions = SessionManager::new(
        store.clone(),
        SessionPolicy::new(SessionDuration::from_millis(1_000)?),
    );
    let _ = sessions.create_session(CreateSession::new(
        session_id.clone(),
        DeviceId::new("device-a")?,
        ConnectorId::new("connector-a")?,
        CapabilitySet::default(),
        ProtocolVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        SessionTimestamp::from_unix_millis(1),
    ))?;
    Ok(session_id)
}

fn transport_events(update: &StateUpdate) -> Vec<TransportEvent> {
    update
        .event()
        .into_iter()
        .flat_map(BridgeStateEvent::changes)
        .filter_map(|change| match change {
            BridgeStateChange::Transport(event) => Some(event.clone()),
            _ => None,
        })
        .collect()
}

fn join<T>(handle: thread::JoinHandle<T>) -> TestResult<T> {
    handle
        .join()
        .map_err(|_| io::Error::other("transport test worker panicked").into())
}

#[test]
fn model_values_are_strongly_typed_and_validated() {
    assert!(matches!(
        TransportId::new(""),
        Err(ref error) if error.kind() == StateIdentifierKind::Transport
    ));
    assert!(matches!(
        ConnectionId::new(""),
        Err(ref error) if error.kind() == StateIdentifierKind::Connection
    ));
    assert_eq!(
        TransportEndpointAddress::new(""),
        Err(TransportModelError::EmptyEndpointAddress)
    );
    assert_eq!(
        TransportCapabilities::new().with_maximum_envelope_size(0),
        Err(TransportModelError::ZeroMaximumEnvelopeSize)
    );
}

#[test]
fn message_endpoint_capabilities_and_statistics_are_transport_independent() -> TestResult {
    let endpoint = endpoint("test://peer")?;
    let capabilities = capabilities()?;
    let session_id = SessionId::new("session-a")?;
    let envelope = TransportMessageEnvelope::new(vec![1_u8, 2, 3])
        .with_session(session_id.clone());
    let statistics = TransportStatistics::new(4, 5, 100, 200);

    assert_eq!(endpoint.transport_id(), &transport_id()?);
    assert_eq!(endpoint.address().as_str(), "test://peer");
    assert_eq!(endpoint.role(), TransportEndpointRole::Peer);
    assert!(capabilities.supports(TransportFeature::ReliableDelivery));
    assert_eq!(capabilities.maximum_envelope_size(), Some(64 * 1024));
    assert_eq!(envelope.session_id(), Some(&session_id));
    assert_eq!(envelope.payload(), &[1, 2, 3]);
    assert_eq!(statistics.messages_sent(), 4);
    assert_eq!(statistics.messages_received(), 5);
    assert_eq!(statistics.bytes_sent(), 100);
    assert_eq!(statistics.bytes_received(), 200);
    Ok(())
}

#[test]
fn create_lookup_list_and_exists_are_consistent_and_deterministic() -> TestResult {
    let manager = manager();
    let created = create(&manager, "connection-z", 10)?;
    let _ = create(&manager, "connection-a", 10)?;
    let _ = create(&manager, "connection-m", 10)?;

    assert_eq!(created.connection().state(), TransportState::Created);
    assert_eq!(created.connection().revision(), TransportRevision::INITIAL);
    assert_eq!(created.connection().created_at(), timestamp(10));
    assert_eq!(created.connection().updated_at(), timestamp(10));
    assert!(manager.connection_exists(&connection_id("connection-a")?)?);
    assert!(!manager.connection_exists(&connection_id("missing")?)?);
    assert!(manager.lookup_connection(&connection_id("missing")?)?.is_none());
    assert_eq!(
        manager
            .list_connections()?
            .iter()
            .map(|connection| connection.connection_id().as_str())
            .collect::<Vec<_>>(),
        vec!["connection-a", "connection-m", "connection-z"]
    );
    Ok(())
}

#[test]
fn duplicate_connections_are_rejected_without_commit() -> TestResult {
    let manager = manager();
    let _ = create(&manager, "duplicate", 1)?;
    let before = manager.state_store().snapshot()?;
    let result = manager.create_connection(CreateTransportConnection::new(
        connection_id("duplicate")?,
        endpoint("test://duplicate")?,
        capabilities()?,
        timestamp(2),
    ));

    assert!(matches!(result, Err(TransportError::DuplicateConnection { .. })));
    assert_eq!(manager.state_store().snapshot()?, before);
    Ok(())
}

#[test]
fn every_valid_lifecycle_transition_commits_and_emits_one_event() -> TestResult {
    let transitions = [
        (TransportState::Created, TransportState::Connecting),
        (TransportState::Created, TransportState::Closing),
        (TransportState::Connecting, TransportState::Connected),
        (TransportState::Connecting, TransportState::Closing),
        (TransportState::Connected, TransportState::Authenticated),
        (TransportState::Connected, TransportState::Closing),
        (TransportState::Authenticated, TransportState::Closing),
        (TransportState::Closing, TransportState::Closed),
    ];

    for (index, (previous, current)) in transitions.into_iter().enumerate() {
        let manager = manager();
        let identifier = format!("valid-{index}");
        insert_connection(&manager, &identifier, previous, 7, 10)?;
        let result = manager.transition_connection(TransitionTransportConnection::new(
            connection_id(identifier)?,
            TransportRevision::new(7),
            current,
            timestamp(11),
        ))?;
        assert_eq!(result.connection().state(), current);
        assert_eq!(result.connection().revision(), TransportRevision::new(8));
        let events = transport_events(result.state_update());
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].kind(),
            TransportEventKind::Lifecycle {
                previous: Some(value),
                current: Some(next),
            } if *value == previous && *next == current
        ));
    }
    Ok(())
}

#[test]
fn every_invalid_lifecycle_transition_rolls_back() -> TestResult {
    let states = [
        TransportState::Created,
        TransportState::Connecting,
        TransportState::Connected,
        TransportState::Authenticated,
        TransportState::Closing,
        TransportState::Closed,
    ];

    for previous in states {
        for requested in states {
            if previous.can_transition_to(requested) {
                continue;
            }
            let manager = manager();
            let identifier = format!("invalid-{previous:?}-{requested:?}");
            insert_connection(&manager, &identifier, previous, 3, 10)?;
            let before = manager.state_store().snapshot()?;
            let result = manager.transition_connection(TransitionTransportConnection::new(
                connection_id(identifier)?,
                TransportRevision::new(3),
                requested,
                timestamp(11),
            ));
            if previous.is_terminal() {
                assert!(matches!(result, Err(TransportError::TerminalConnection { .. })));
            } else {
                assert!(matches!(result, Err(TransportError::InvalidTransition { .. })));
            }
            assert_eq!(manager.state_store().snapshot()?, before);
        }
    }
    Ok(())
}

#[test]
fn close_advances_to_closing_then_closed_and_rejects_terminal_reuse() -> TestResult {
    let manager = manager();
    let created = create(&manager, "close", 1)?;
    let closing = manager.close_connection(CloseTransportConnection::new(
        connection_id("close")?,
        created.connection().revision(),
        timestamp(2),
    ))?;
    let closed = manager.close_connection(CloseTransportConnection::new(
        connection_id("close")?,
        closing.connection().revision(),
        timestamp(3),
    ))?;

    assert_eq!(closing.connection().state(), TransportState::Closing);
    assert_eq!(closed.connection().state(), TransportState::Closed);
    assert!(matches!(
        manager.close_connection(CloseTransportConnection::new(
            connection_id("close")?,
            closed.connection().revision(),
            timestamp(4),
        )),
        Err(TransportError::TerminalConnection { .. })
    ));
    Ok(())
}

#[test]
fn bind_and_unbind_existing_session_publish_typed_events() -> TestResult {
    let manager = manager();
    let session_id = add_session(&manager.state_store())?;
    insert_connection(&manager, "binding", TransportState::Authenticated, 4, 10)?;

    let bound = manager.bind_session(BindTransportSession::new(
        connection_id("binding")?,
        TransportRevision::new(4),
        session_id.clone(),
        timestamp(11),
    ))?;
    assert_eq!(bound.connection().session_id(), Some(&session_id));
    assert!(matches!(
        transport_events(bound.state_update())[0].kind(),
        TransportEventKind::SessionBinding {
            previous: None,
            current: Some(value),
        } if value == &session_id
    ));

    let unbound = manager.unbind_session(UnbindTransportSession::new(
        connection_id("binding")?,
        bound.connection().revision(),
        timestamp(12),
    ))?;
    assert_eq!(unbound.connection().session_id(), None);
    assert!(matches!(
        transport_events(unbound.state_update())[0].kind(),
        TransportEventKind::SessionBinding {
            previous: Some(value),
            current: None,
        } if value == &session_id
    ));
    Ok(())
}

#[test]
fn binding_validation_is_structured_and_atomic() -> TestResult {
    let manager = manager();
    let session_id = add_session(&manager.state_store())?;
    let created = create(&manager, "binding-errors", 10)?;
    let before = manager.state_store().snapshot()?;

    assert!(matches!(
        manager.bind_session(BindTransportSession::new(
            connection_id("binding-errors")?,
            created.connection().revision(),
            session_id.clone(),
            timestamp(11),
        )),
        Err(TransportError::BindingRequiresAuthenticated { .. })
    ));
    assert_eq!(manager.state_store().snapshot()?, before);

    insert_connection(&manager, "missing-session", TransportState::Authenticated, 1, 10)?;
    assert!(matches!(
        manager.bind_session(BindTransportSession::new(
            connection_id("missing-session")?,
            TransportRevision::new(1),
            SessionId::new("absent")?,
            timestamp(11),
        )),
        Err(TransportError::MissingSession { .. })
    ));

    insert_connection(&manager, "already-bound", TransportState::Authenticated, 1, 10)?;
    let bound = manager.bind_session(BindTransportSession::new(
        connection_id("already-bound")?,
        TransportRevision::new(1),
        session_id.clone(),
        timestamp(11),
    ))?;
    assert!(matches!(
        manager.bind_session(BindTransportSession::new(
            connection_id("already-bound")?,
            bound.connection().revision(),
            session_id,
            timestamp(12),
        )),
        Err(TransportError::SessionAlreadyBound { .. })
    ));

    insert_connection(&manager, "never-bound", TransportState::Authenticated, 1, 10)?;
    assert!(matches!(
        manager.unbind_session(UnbindTransportSession::new(
            connection_id("never-bound")?,
            TransportRevision::new(1),
            timestamp(11),
        )),
        Err(TransportError::SessionNotBound { .. })
    ));
    Ok(())
}

#[test]
fn missing_stale_and_regressing_operations_do_not_commit_or_emit() -> TestResult {
    let manager = manager();
    let created = create(&manager, "validation", 10)?;
    let subscription = manager.state_store().subscribe()?;
    let before = manager.state_store().snapshot()?;

    assert!(matches!(
        manager.transition_connection(TransitionTransportConnection::new(
            connection_id("missing")?,
            TransportRevision::INITIAL,
            TransportState::Connecting,
            timestamp(11),
        )),
        Err(TransportError::ConnectionNotFound { .. })
    ));
    assert!(matches!(
        manager.transition_connection(TransitionTransportConnection::new(
            connection_id("validation")?,
            TransportRevision::new(99),
            TransportState::Connecting,
            timestamp(11),
        )),
        Err(TransportError::StaleRevision { .. })
    ));
    assert!(matches!(
        manager.transition_connection(TransitionTransportConnection::new(
            connection_id("validation")?,
            created.connection().revision(),
            TransportState::Connecting,
            timestamp(9),
        )),
        Err(TransportError::TimestampRegression { .. })
    ));
    assert_eq!(subscription.try_recv(), Err(StateReceiveError::Empty));
    assert_eq!(manager.state_store().snapshot()?, before);
    Ok(())
}

#[test]
fn state_revisions_snapshots_and_creation_events_are_consistent() -> TestResult {
    let manager = manager();
    let before = manager.state_store().snapshot()?;
    let created = create(&manager, "events", 10)?;
    let old_snapshot = created.state_update().snapshot().clone();
    let event = transport_events(created.state_update())
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("missing transport creation event"))?;

    assert_eq!(created.state_update().snapshot().revision().get(), before.revision().get() + 1);
    assert_eq!(event.connection_id(), &connection_id("events")?);
    assert_eq!(event.transport_id(), &transport_id()?);
    assert_eq!(event.connection_revision(), TransportRevision::INITIAL);
    assert_eq!(event.timestamp(), timestamp(10));
    assert!(matches!(
        event.kind(),
        TransportEventKind::Lifecycle {
            previous: None,
            current: Some(TransportState::Created),
        }
    ));

    let _ = manager.transition_connection(TransitionTransportConnection::new(
        connection_id("events")?,
        created.connection().revision(),
        TransportState::Connecting,
        timestamp(11),
    ))?;
    assert_eq!(
        old_snapshot
            .connections()
            .get(&connection_id("events")?)
            .map(TransportConnectionSnapshot::state),
        Some(TransportState::Created)
    );
    Ok(())
}

#[test]
fn connection_and_event_order_are_deterministic() -> TestResult {
    let manager = manager();
    let update = manager.state_store().update(|draft| {
        for identifier in ["z-connection", "a-connection", "m-connection"] {
            let connection_id = ConnectionId::new(identifier)
                .map_err(|error| StateError::rejected("transport_test", error.to_string()))?;
            let transport_id = TransportId::new("test-transport")
                .map_err(|error| StateError::rejected("transport_test", error.to_string()))?;
            let endpoint = TransportEndpoint::new(
                transport_id.clone(),
                TransportEndpointAddress::new(format!("test://{identifier}"))
                    .map_err(|error| StateError::rejected("transport_test", error.to_string()))?,
                TransportEndpointRole::Peer,
            );
            let _ = draft.connections_mut().insert(
                TransportConnectionSnapshot::from_parts(TransportConnectionParts {
                    connection_id,
                    transport_id,
                    endpoint,
                    capabilities: TransportCapabilities::new(),
                    state: TransportState::Created,
                    session_id: None,
                    revision: TransportRevision::INITIAL,
                    created_at: timestamp(1),
                    updated_at: timestamp(1),
                }),
            )?;
        }
        Ok(())
    })?;

    assert_eq!(
        update
            .snapshot()
            .connections()
            .keys()
            .map(ConnectionId::as_str)
            .collect::<Vec<_>>(),
        vec!["a-connection", "m-connection", "z-connection"]
    );
    assert_eq!(
        transport_events(&update)
            .iter()
            .map(|event| event.connection_id().as_str())
            .collect::<Vec<_>>(),
        vec!["a-connection", "m-connection", "z-connection"]
    );
    Ok(())
}

#[test]
fn identical_inputs_produce_identical_snapshots_and_events() -> TestResult {
    let first = manager();
    let second = manager();
    let first_created = create(&first, "deterministic", 10)?;
    let second_created = create(&second, "deterministic", 10)?;
    let first_updated = first.transition_connection(TransitionTransportConnection::new(
        connection_id("deterministic")?,
        first_created.connection().revision(),
        TransportState::Connecting,
        timestamp(11),
    ))?;
    let second_updated = second.transition_connection(TransitionTransportConnection::new(
        connection_id("deterministic")?,
        second_created.connection().revision(),
        TransportState::Connecting,
        timestamp(11),
    ))?;

    assert_eq!(first.state_store().snapshot()?, second.state_store().snapshot()?);
    assert_eq!(first_updated.state_update().event(), second_updated.state_update().event());
    Ok(())
}

#[test]
fn concurrent_creation_and_lookup_are_consistent() -> TestResult {
    let manager = Arc::new(manager());
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();

    for index in 0..8 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> TransportResult<()> {
            barrier.wait();
            let identifier = ConnectionId::new(format!("concurrent-{index}"))
                .map_err(|error| TransportError::state_invariant(error.to_string()))?;
            let endpoint = TransportEndpoint::new(
                TransportId::new("test-transport")
                    .map_err(|error| TransportError::state_invariant(error.to_string()))?,
                TransportEndpointAddress::new(format!("test://{index}"))?,
                TransportEndpointRole::Peer,
            );
            let result = manager.create_connection(CreateTransportConnection::new(
                identifier.clone(),
                endpoint,
                TransportCapabilities::new(),
                timestamp(1),
            ))?;
            let found = manager.lookup_connection(&identifier)?.ok_or_else(|| {
                TransportError::state_invariant("created connection was not found")
            })?;
            if found.revision() != result.connection().revision() {
                return Err(TransportError::state_invariant(
                    "lookup observed an inconsistent revision",
                ));
            }
            Ok(())
        }));
    }

    barrier.wait();
    for worker in workers {
        join(worker)??;
    }
    assert_eq!(manager.state_store().snapshot()?.connections().len(), 8);
    assert_eq!(manager.state_store().snapshot()?.revision().get(), 8);
    Ok(())
}

#[test]
fn concurrent_close_allows_one_revision_winner() -> TestResult {
    let manager = Arc::new(manager());
    let created = create(&manager, "concurrent-close", 1)?;
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        let revision = created.connection().revision();
        workers.push(thread::spawn(move || {
            barrier.wait();
            ConnectionId::new("concurrent-close")
                .map_err(|error| TransportError::state_invariant(error.to_string()))
                .and_then(|identifier| {
                    manager.close_connection(CloseTransportConnection::new(
                        identifier,
                        revision,
                        timestamp(2),
                    ))
                })
        }));
    }

    barrier.wait();
    let results = workers
        .into_iter()
        .map(join)
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(TransportError::StaleRevision { .. })))
            .count(),
        1
    );
    assert_eq!(
        manager
            .lookup_connection(&connection_id("concurrent-close")?)?
            .map(|connection| connection.state()),
        Some(TransportState::Closing)
    );
    Ok(())
}

#[test]
fn public_transport_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TransportManager>();
    assert_send_sync::<TransportConnectionSnapshot>();
    assert_send_sync::<TransportMutation>();
    assert_send_sync::<TransportEndpoint>();
    assert_send_sync::<TransportMessageEnvelope>();
    assert_send_sync::<TransportStatistics>();
}

#[derive(Debug)]
struct TestConnection {
    connection_id: ConnectionId,
    transport_id: TransportId,
    endpoint: TransportEndpoint,
    capabilities: TransportCapabilities,
}

impl TransportConnection for TestConnection {
    fn connection_id(&self) -> &ConnectionId {
        &self.connection_id
    }

    fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    fn endpoint(&self) -> &TransportEndpoint {
        &self.endpoint
    }

    fn capabilities(&self) -> &TransportCapabilities {
        &self.capabilities
    }

    fn statistics(&self) -> TransportStatistics {
        TransportStatistics::default()
    }

    fn send(&self, _envelope: TransportMessageEnvelope) -> TransportFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn receive(&self) -> TransportFuture<'_, TransportMessageEnvelope> {
        Box::pin(async { Ok(TransportMessageEnvelope::new(Vec::<u8>::new())) })
    }

    fn close(&self) -> TransportFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug)]
struct TestFactory {
    transport_id: TransportId,
    capabilities: TransportCapabilities,
}

impl TransportFactory for TestFactory {
    fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    fn capabilities(&self) -> &TransportCapabilities {
        &self.capabilities
    }

    fn create(
        &self,
        connection_id: ConnectionId,
        endpoint: TransportEndpoint,
    ) -> TransportFuture<'_, Arc<dyn TransportConnection>> {
        let connection = TestConnection {
            connection_id,
            transport_id: self.transport_id.clone(),
            endpoint,
            capabilities: self.capabilities.clone(),
        };
        Box::pin(async move { Ok(Arc::new(connection) as Arc<dyn TransportConnection>) })
    }
}

fn poll_ready<T>(future: TransportFuture<'_, T>) -> TransportResult<T> {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = future;
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => Err(TransportError::state_invariant(
            "test transport future unexpectedly returned Pending",
        )),
    }
}

#[test]
fn transport_interfaces_are_object_safe_and_runtime_independent() -> TestResult {
    let factory: Arc<dyn TransportFactory> = Arc::new(TestFactory {
        transport_id: transport_id()?,
        capabilities: capabilities()?,
    });
    let connection = poll_ready(factory.create(
        connection_id("interface")?,
        endpoint("test://interface")?,
    ))?;

    assert_eq!(connection.connection_id(), &connection_id("interface")?);
    assert_eq!(connection.transport_id(), factory.transport_id());
    poll_ready(connection.send(TransportMessageEnvelope::new(vec![1_u8])))?;
    assert_eq!(poll_ready(connection.receive())?.payload(), &[]);
    poll_ready(connection.close())?;
    Ok(())
}
