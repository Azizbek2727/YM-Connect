use std::{
    error::Error,
    future::Future,
    io,
    sync::{Arc, Barrier},
    task::{Context, Poll, Wake, Waker},
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

fn session_id() -> TestResult<SessionId> {
    Ok(SessionId::new("session-a")?)
}

fn timestamp(value: u64) -> TransportTimestamp {
    TransportTimestamp::from_unix_millis(value)
}

fn endpoint() -> TestResult<TransportEndpoint> {
    Ok(TransportEndpoint::new(
        transport_id()?,
        TransportEndpointAddress::new("test://endpoint")?,
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

fn manager_with_session() -> TestResult<(TransportManager, BridgeStateStore)> {
    let store = BridgeStateStore::default();
    store.update(|draft| {
        let _ = draft.devices_mut().insert(DeviceDescriptor {
            device_id: "device-a".to_owned(),
            display_name: "Device A".to_owned(),
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
        session_id()?,
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
    Ok((TransportManager::new(store.clone()), store))
}

fn create_command(identifier: impl Into<String>, at: u64) -> TestResult<CreateTransportConnection> {
    Ok(CreateTransportConnection::new(
        connection_id(identifier)?,
        endpoint()?,
        capabilities()?,
        timestamp(at),
    ))
}

fn create(
    manager: &TransportManager,
    identifier: impl Into<String>,
    at: u64,
) -> TestResult<TransportMutation> {
    Ok(manager.create_connection(create_command(identifier, at)?)?)
}

fn transition_to_authenticated(
    manager: &TransportManager,
    identifier: &str,
    starting_revision: TransportRevision,
    starting_time: u64,
) -> TestResult<TransportMutation> {
    let connecting = manager.transition_connection(TransitionTransportConnection::new(
        connection_id(identifier)?,
        starting_revision,
        TransportState::Connecting,
        timestamp(starting_time),
    ))?;
    let connected = manager.transition_connection(TransitionTransportConnection::new(
        connection_id(identifier)?,
        connecting.connection().revision(),
        TransportState::Connected,
        timestamp(starting_time + 1),
    ))?;
    Ok(manager.transition_connection(TransitionTransportConnection::new(
        connection_id(identifier)?,
        connected.connection().revision(),
        TransportState::Authenticated,
        timestamp(starting_time + 2),
    ))?)
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
fn strong_identifiers_and_model_validation_are_structured() -> TestResult {
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
    Ok(())
}

#[test]
fn endpoint_capabilities_envelope_and_statistics_are_transport_independent() -> TestResult {
    let endpoint = endpoint()?;
    let capabilities = capabilities()?;
    let envelope = TransportMessageEnvelope::new(vec![1_u8, 2, 3]).with_session(session_id()?);
    let statistics = TransportStatistics::new(4, 5, 100, 200);

    assert_eq!(endpoint.transport_id(), &transport_id()?);
    assert_eq!(endpoint.address().as_str(), "test://endpoint");
    assert_eq!(endpoint.role(), TransportEndpointRole::Peer);
    assert!(capabilities.supports(TransportFeature::ReliableDelivery));
    assert_eq!(capabilities.maximum_envelope_size(), Some(64 * 1024));
    assert_eq!(envelope.payload(), &[1, 2, 3]);
    assert_eq!(envelope.session_id(), Some(&session_id()?));
    assert_eq!(statistics.messages_sent(), 4);
    assert_eq!(statistics.messages_received(), 5);
    assert_eq!(statistics.bytes_sent(), 100);
    assert_eq!(statistics.bytes_received(), 200);
    Ok(())
}

#[test]
fn valid_connection_creation_commits_created_state() -> TestResult {
    let manager = manager();
    let before = manager.state_store().snapshot()?;
    let result = create(&manager, "connection-a", 10)?;

    assert_eq!(result.connection().connection_id(), &connection_id("connection-a")?);
    assert_eq!(result.connection().state(), TransportState::Created);
    assert_eq!(result.connection().revision(), TransportRevision::INITIAL);
    assert_eq!(result.connection().created_at(), timestamp(10));
    assert_eq!(result.connection().updated_at(), timestamp(10));
    assert_eq!(result.connection().session_id(), None);
    assert_eq!(
        result.state_update().snapshot().revision().get(),
        before.revision().get() + 1
    );
    assert_eq!(result.state_update().snapshot().connections().len(), 1);
    Ok(())
}

#[test]
fn duplicate_connection_identifiers_are_rejected_without_commit() -> TestResult {
    let manager = manager();
    let _ = create(&manager, "connection-a", 10)?;
    let before = manager.state_store().snapshot()?;
    let result = manager.create_connection(create_command("connection-a", 11)?);

    assert!(matches!(
        result,
        Err(TransportError::DuplicateConnection { ref connection_id })
            if connection_id.as_str() == "connection-a"
    ));
    assert_eq!(manager.state_store().snapshot()?, before);
    Ok(())
}

#[test]
fn every_allowed_lifecycle_transition_succeeds() -> TestResult {
    let allowed = [
        (TransportState::Created, TransportState::Connecting),
        (TransportState::Created, TransportState::Closing),
        (TransportState::Connecting, TransportState::Connected),
        (TransportState::Connecting, TransportState::Closing),
        (TransportState::Connected, TransportState::Authenticated),
        (TransportState::Connected, TransportState::Closing),
        (TransportState::Authenticated, TransportState::Closing),
        (TransportState::Closing, TransportState::Closed),
    ];

    for (index, (previous, current)) in allowed.into_iter().enumerate() {
        let manager = manager();
        let identifier = format!("allowed-{index}");
        let created = create(&manager, identifier.clone(), 1)?;
        let mut mutation = created;
        let path = match previous {
            TransportState::Created => Vec::new(),
            TransportState::Connecting => vec![TransportState::Connecting],
            TransportState::Connected => {
                vec![TransportState::Connecting, TransportState::Connected]
            }
            TransportState::Authenticated => vec![
                TransportState::Connecting,
                TransportState::Connected,
                TransportState::Authenticated,
            ],
            TransportState::Closing => vec![TransportState::Closing],
            TransportState::Closed => unreachable!(),
        };
        for (step, state) in path.into_iter().enumerate() {
            mutation = manager.transition_connection(TransitionTransportConnection::new(
                connection_id(identifier.clone())?,
                mutation.connection().revision(),
                state,
                timestamp((step + 2) as u64),
            ))?;
        }
        let updated = manager.transition_connection(TransitionTransportConnection::new(
            connection_id(identifier)?,
            mutation.connection().revision(),
            current,
            timestamp(10),
        ))?;
        assert_eq!(updated.connection().state(), current);
        let events = transport_events(updated.state_update());
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
fn every_invalid_lifecycle_transition_is_rejected() -> TestResult {
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
            let created = create(&manager, identifier.clone(), 1)?;
            let mut mutation = created;
            let path = match previous {
                TransportState::Created => Vec::new(),
                TransportState::Connecting => vec![TransportState::Connecting],
                TransportState::Connected => {
                    vec![TransportState::Connecting, TransportState::Connected]
                }
                TransportState::Authenticated => vec![
                    TransportState::Connecting,
                    TransportState::Connected,
                    TransportState::Authenticated,
                ],
                TransportState::Closing => vec![TransportState::Closing],
                TransportState::Closed => vec![TransportState::Closing, TransportState::Closed],
            };
            for (step, state) in path.into_iter().enumerate() {
                mutation = manager.transition_connection(TransitionTransportConnection::new(
                    connection_id(identifier.clone())?,
                    mutation.connection().revision(),
                    state,
                    timestamp((step + 2) as u64),
                ))?;
            }
            let before = manager.state_store().snapshot()?;
            let result = manager.transition_connection(TransitionTransportConnection::new(
                connection_id(identifier)?,
                mutation.connection().revision(),
                requested,
                timestamp(20),
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
fn repeated_transitions_and_terminal_connections_are_rejected() -> TestResult {
    let manager = manager();
    let created = create(&manager, "repeat", 1)?;
    let connecting = manager.transition_connection(TransitionTransportConnection::new(
        connection_id("repeat")?,
        created.connection().revision(),
        TransportState::Connecting,
        timestamp(2),
    ))?;
    assert!(matches!(
        manager.transition_connection(TransitionTransportConnection::new(
            connection_id("repeat")?,
            connecting.connection().revision(),
            TransportState::Connecting,
            timestamp(3),
        )),
        Err(TransportError::InvalidTransition { .. })
    ));

    let closing = manager.close_connection(CloseTransportConnection::new(
        connection_id("repeat")?,
        connecting.connection().revision(),
        timestamp(4),
    ))?;
    let closed = manager.close_connection(CloseTransportConnection::new(
        connection_id("repeat")?,
        closing.connection().revision(),
        timestamp(5),
    ))?;
    assert_eq!(closed.connection().state(), TransportState::Closed);
    assert!(matches!(
        manager.close_connection(CloseTransportConnection::new(
            connection_id("repeat")?,
            closed.connection().revision(),
            timestamp(6),
        )),
        Err(TransportError::TerminalConnection { .. })
    ));
    Ok(())
}

#[test]
fn authenticated_connection_binds_and_unbinds_existing_session() -> TestResult {
    let (manager, _) = manager_with_session()?;
    let created = create(&manager, "binding", 10)?;
    let authenticated = transition_to_authenticated(
        &manager,
        "binding",
        created.connection().revision(),
        11,
    )?;
    let expected_session = session_id()?;
    let bound = manager.bind_session(BindTransportSession::new(
        connection_id("binding")?,
        authenticated.connection().revision(),
        expected_session.clone(),
        timestamp(14),
    ))?;
    assert_eq!(bound.connection().session_id(), Some(&expected_session));
    assert!(matches!(
        transport_events(bound.state_update())[0].kind(),
        TransportEventKind::SessionBinding {
            previous: None,
            current: Some(value),
        } if value == &expected_session
    ));

    let unbound = manager.unbind_session(UnbindTransportSession::new(
        connection_id("binding")?,
        bound.connection().revision(),
        timestamp(15),
    ))?;
    assert_eq!(unbound.connection().session_id(), None);
    assert!(matches!(
        transport_events(unbound.state_update())[0].kind(),
        TransportEventKind::SessionBinding {
            previous: Some(value),
            current: None,
        } if value == &expected_session
    ));
    Ok(())
}

#[test]
fn binding_validates_state_session_and_existing_binding() -> TestResult {
    let (manager, store) = manager_with_session()?;
    let created = create(&manager, "binding-errors", 10)?;
    let before = store.snapshot()?;
    assert!(matches!(
        manager.bind_session(BindTransportSession::new(
            connection_id("binding-errors")?,
            created.connection().revision(),
            session_id()?,
            timestamp(11),
        )),
        Err(TransportError::BindingRequiresAuthenticated { .. })
    ));
    assert_eq!(store.snapshot()?, before);

    let authenticated = transition_to_authenticated(
        &manager,
        "binding-errors",
        created.connection().revision(),
        12,
    )?;
    assert!(matches!(
        manager.bind_session(BindTransportSession::new(
            connection_id("binding-errors")?,
            authenticated.connection().revision(),
            SessionId::new("missing-session")?,
            timestamp(15),
        )),
        Err(TransportError::MissingSession { .. })
    ));

    let bound = manager.bind_session(BindTransportSession::new(
        connection_id("binding-errors")?,
        authenticated.connection().revision(),
        session_id()?,
        timestamp(16),
    ))?;
    assert!(matches!(
        manager.bind_session(BindTransportSession::new(
            connection_id("binding-errors")?,
            bound.connection().revision(),
            session_id()?,
            timestamp(17),
        )),
        Err(TransportError::SessionAlreadyBound { .. })
    ));
    Ok(())
}

#[test]
fn unbind_requires_existing_binding() -> TestResult {
    let manager = manager();
    let created = create(&manager, "unbound", 10)?;
    let before = manager.state_store().snapshot()?;
    assert!(matches!(
        manager.unbind_session(UnbindTransportSession::new(
            connection_id("unbound")?,
            created.connection().revision(),
            timestamp(11),
        )),
        Err(TransportError::SessionNotBound { .. })
    ));
    assert_eq!(manager.state_store().snapshot()?, before);
    Ok(())
}

#[test]
fn close_api_advances_through_closing_and_closed() -> TestResult {
    let manager = manager();
    let created = create(&manager, "close", 1)?;
    let closing = manager.close_connection(CloseTransportConnection::new(
        connection_id("close")?,
        created.connection().revision(),
        timestamp(2),
    ))?;
    assert_eq!(closing.connection().state(), TransportState::Closing);
    let closed = manager.close_connection(CloseTransportConnection::new(
        connection_id("close")?,
        closing.connection().revision(),
        timestamp(3),
    ))?;
    assert_eq!(closed.connection().state(), TransportState::Closed);
    Ok(())
}

#[test]
fn lookup_list_and_exists_are_deterministic() -> TestResult {
    let manager = manager();
    let _ = create(&manager, "connection-z", 1)?;
    let _ = create(&manager, "connection-a", 1)?;
    let _ = create(&manager, "connection-m", 1)?;

    assert!(manager.connection_exists(&connection_id("connection-a")?)?);
    assert!(!manager.connection_exists(&connection_id("missing")?)?);
    assert_eq!(
        manager
            .lookup_connection(&connection_id("connection-m")?)?
            .map(|connection| connection.connection_id().as_str().to_owned()),
        Some("connection-m".to_owned())
    );
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
fn stale_revision_timestamp_regression_and_missing_connection_are_structured() -> TestResult {
    let manager = manager();
    let created = create(&manager, "validation", 10)?;
    let before = manager.state_store().snapshot()?;

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
    assert!(matches!(
        manager.transition_connection(TransitionTransportConnection::new(
            connection_id("missing")?,
            TransportRevision::INITIAL,
            TransportState::Connecting,
            timestamp(11),
        )),
        Err(TransportError::ConnectionNotFound { .. })
    ));
    assert_eq!(manager.state_store().snapshot()?, before);
    Ok(())
}

#[test]
fn lifecycle_and_binding_events_contain_typed_fields() -> TestResult {
    let (manager, _) = manager_with_session()?;
    let created = create(&manager, "events", 10)?;
    let event = transport_events(created.state_update())
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("missing creation event"))?;
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

    let authenticated = transition_to_authenticated(
        &manager,
        "events",
        created.connection().revision(),
        11,
    )?;
    let bound = manager.bind_session(BindTransportSession::new(
        connection_id("events")?,
        authenticated.connection().revision(),
        session_id()?,
        timestamp(14),
    ))?;
    assert_eq!(transport_events(bound.state_update()).len(), 1);
    Ok(())
}

#[test]
fn rejected_operations_emit_no_event_and_roll_back_completely() -> TestResult {
    let manager = manager();
    let created = create(&manager, "rollback", 10)?;
    let subscription = manager.state_store().subscribe()?;
    let before = manager.state_store().snapshot()?;
    let result = manager.transition_connection(TransitionTransportConnection::new(
        connection_id("rollback")?,
        created.connection().revision(),
        TransportState::Authenticated,
        timestamp(11),
    ));

    assert!(matches!(result, Err(TransportError::InvalidTransition { .. })));
    assert_eq!(subscription.try_recv(), Err(StateReceiveError::Empty));
    assert_eq!(manager.state_store().snapshot()?, before);
    assert_eq!(
        manager
            .lookup_connection(&connection_id("rollback")?)?
            .map(|connection| connection.state()),
        Some(TransportState::Created)
    );
    Ok(())
}

#[test]
fn snapshots_remain_immutable_across_connection_updates() -> TestResult {
    let manager = manager();
    let created = create(&manager, "snapshot", 10)?;
    let before = manager.state_store().snapshot()?;
    let _ = manager.transition_connection(TransitionTransportConnection::new(
        connection_id("snapshot")?,
        created.connection().revision(),
        TransportState::Connecting,
        timestamp(11),
    ))?;

    assert_eq!(
        before
            .connections()
            .get(&connection_id("snapshot")?)
            .map(TransportConnectionSnapshot::state),
        Some(TransportState::Created)
    );
    assert_eq!(
        manager
            .state_store()
            .snapshot()?
            .connections()
            .get(&connection_id("snapshot")?)
            .map(TransportConnectionSnapshot::state),
        Some(TransportState::Connecting)
    );
    Ok(())
}

#[test]
fn connection_registry_and_event_order_are_deterministic() -> TestResult {
    let store = BridgeStateStore::default();
    let update = store.update(|draft| {
        for identifier in ["z-connection", "a-connection", "m-connection"] {
            let endpoint = endpoint().map_err(|error| {
                StateError::rejected("transport_test", error.to_string())
            })?;
            let capabilities = capabilities().map_err(|error| {
                StateError::rejected("transport_test", error.to_string())
            })?;
            let connection_id = ConnectionId::new(identifier).map_err(|error| {
                StateError::rejected("transport_test", error.to_string())
            })?;
            let transport_id = endpoint.transport_id().clone();
            let _ = draft.connections_mut().insert(
                TransportConnectionSnapshot::from_parts(TransportConnectionParts {
                    connection_id,
                    transport_id,
                    endpoint,
                    capabilities,
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
    assert_eq!(
        first_updated.state_update().event(),
        second_updated.state_update().event()
    );
    Ok(())
}

#[test]
fn concurrent_connection_creation_is_serialized() -> TestResult {
    let manager = Arc::new(manager());
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();

    for index in 0..8 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> TransportResult<()> {
            barrier.wait();
            let connection_id = ConnectionId::new(format!("concurrent-{index}"))
                .map_err(|error| TransportError::state_invariant(error.to_string()))?;
            let transport_id = TransportId::new("test-transport")
                .map_err(|error| TransportError::state_invariant(error.to_string()))?;
            let address = TransportEndpointAddress::new(format!("test://{index}"))?;
            let endpoint = TransportEndpoint::new(
                transport_id,
                address,
                TransportEndpointRole::Peer,
            );
            let _ = manager.create_connection(CreateTransportConnection::new(
                connection_id,
                endpoint,
                TransportCapabilities::new(),
                timestamp(1),
            ))?;
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
fn concurrent_close_uses_optimistic_revision_consistency() -> TestResult {
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
            let identifier = ConnectionId::new("concurrent-close");
            match identifier {
                Ok(identifier) => manager.close_connection(CloseTransportConnection::new(
                    identifier,
                    revision,
                    timestamp(2),
                )),
                Err(error) => Err(TransportError::state_invariant(error.to_string())),
            }
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
fn transport_manager_and_public_records_are_send_and_sync() {
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

    fn send<'a>(&'a self, _envelope: TransportMessageEnvelope) -> TransportFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn receive<'a>(&'a self) -> TransportFuture<'a, TransportMessageEnvelope> {
        Box::pin(async { Ok(TransportMessageEnvelope::new(Vec::<u8>::new())) })
    }

    fn close<'a>(&'a self) -> TransportFuture<'a, ()> {
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

    fn create<'a>(
        &'a self,
        connection_id: ConnectionId,
        endpoint: TransportEndpoint,
    ) -> TransportFuture<'a, Arc<dyn TransportConnection>> {
        let connection = TestConnection {
            connection_id,
            transport_id: self.transport_id.clone(),
            endpoint,
            capabilities: self.capabilities.clone(),
        };
        Box::pin(async move { Ok(Arc::new(connection) as Arc<dyn TransportConnection>) })
    }
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn poll_ready<T>(future: TransportFuture<'_, T>) -> TransportResult<T> {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = future;
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => Err(TransportError::state_invariant(
            "test transport future unexpectedly returned Pending",
        )),
    }
}

#[test]
fn factory_and_connection_interfaces_are_object_safe_and_runtime_independent() -> TestResult {
    let factory: Arc<dyn TransportFactory> = Arc::new(TestFactory {
        transport_id: transport_id()?,
        capabilities: capabilities()?,
    });
    let connection = poll_ready(factory.create(connection_id("interface")?, endpoint()?))?;
    assert_eq!(connection.connection_id(), &connection_id("interface")?);
    assert_eq!(connection.transport_id(), factory.transport_id());
    poll_ready(connection.send(TransportMessageEnvelope::new(vec![1_u8])))?;
    assert_eq!(poll_ready(connection.receive())?.payload(), &[]);
    poll_ready(connection.close())?;
    Ok(())
}
