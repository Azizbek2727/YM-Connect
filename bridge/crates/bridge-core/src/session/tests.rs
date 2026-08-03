use std::{
    error::Error,
    io,
    sync::{Arc, Barrier},
    thread,
};

use ym_connect_protocol::v1::{
    BrowserDescriptor, Capability, CapabilitySet, DeviceDescriptor, ProtocolVersion,
};

use crate::*;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const TIMEOUT_MS: u64 = 100;

fn timestamp(value: u64) -> SessionTimestamp {
    SessionTimestamp::from_unix_millis(value)
}

fn protocol_version() -> ProtocolVersion {
    ProtocolVersion {
        major: 1,
        minor: 2,
        patch: 3,
    }
}

fn capabilities() -> CapabilitySet {
    CapabilitySet {
        supported: vec![
            Capability::CapabilityPlay as i32,
            Capability::CapabilityPause as i32,
        ],
        required: vec![Capability::CapabilityPlay as i32],
        ..CapabilitySet::default()
    }
}

fn device(index: usize) -> TestResult<DeviceId> {
    Ok(DeviceId::new(format!("device-{index:02}"))?)
}

fn connector(index: usize) -> TestResult<ConnectorId> {
    Ok(ConnectorId::new(format!("connector-{index:02}"))?)
}

fn session_id(value: impl Into<String>) -> TestResult<SessionId> {
    Ok(SessionId::new(value.into())?)
}

fn setup(pair_count: usize) -> TestResult<(SessionManager, BridgeStateStore)> {
    let store = BridgeStateStore::default();
    store.update(|draft| {
        for index in 0..pair_count {
            let _ = draft.devices_mut().insert(DeviceDescriptor {
                device_id: format!("device-{index:02}"),
                display_name: format!("Device {index}"),
                ..DeviceDescriptor::default()
            })?;
            let _ = draft.connectors_mut().insert(BrowserDescriptor {
                connector_id: format!("connector-{index:02}"),
                ..BrowserDescriptor::default()
            })?;
        }
        Ok(())
    })?;
    let policy = SessionPolicy::new(SessionDuration::from_millis(TIMEOUT_MS)?);
    Ok((SessionManager::new(store.clone(), policy), store))
}

fn create_command(
    identifier: impl Into<String>,
    association_index: usize,
    at: u64,
) -> TestResult<CreateSession> {
    Ok(CreateSession::new(
        session_id(identifier)?,
        device(association_index)?,
        connector(association_index)?,
        capabilities(),
        protocol_version(),
        timestamp(at),
    ))
}

fn create(
    manager: &SessionManager,
    identifier: impl Into<String>,
    association_index: usize,
    at: u64,
) -> TestResult<SessionMutation> {
    Ok(manager.create_session(create_command(
        identifier,
        association_index,
        at,
    )?)?)
}

fn restore_command(
    identifier: impl Into<String>,
    association_index: usize,
    lifecycle: SessionLifecycleState,
    revision: u64,
) -> TestResult<RestoreSession> {
    Ok(RestoreSession::new(
        session_id(identifier)?,
        device(association_index)?,
        connector(association_index)?,
        capabilities(),
        protocol_version(),
        lifecycle,
        SessionRevision::new(revision),
        timestamp(1),
        timestamp(2),
        timestamp(3),
    ))
}

fn transition(update: &StateUpdate) -> TestResult<SessionStateTransition> {
    update
        .event()
        .and_then(|event| {
            event.changes().iter().find_map(|change| match change {
                BridgeStateChange::SessionLifecycle(transition) => Some(transition.clone()),
                _ => None,
            })
        })
        .ok_or_else(|| io::Error::other("missing session lifecycle transition").into())
}

fn join<T>(handle: thread::JoinHandle<T>) -> TestResult<T> {
    handle
        .join()
        .map_err(|_| io::Error::other("session test worker panicked").into())
}

#[test]
fn valid_session_creation_commits_created_state() -> TestResult {
    let (manager, store) = setup(1)?;
    let result = create(&manager, "session-a", 0, 10)?;

    assert_eq!(result.session().session_id(), &session_id("session-a")?);
    assert_eq!(result.session().created_at(), timestamp(10));
    assert_eq!(result.session().last_activity_at(), timestamp(10));
    assert_eq!(result.session().lifecycle(), SessionLifecycleState::Created);
    assert_eq!(result.session().revision(), SessionRevision::INITIAL);
    assert_eq!(result.session().device_id(), &device(0)?);
    assert_eq!(result.session().connector_id(), &connector(0)?);
    assert_eq!(result.session().protocol_version(), &protocol_version());
    assert_eq!(store.snapshot()?.revision().get(), 2);
    assert_eq!(store.snapshot()?.sessions().len(), 1);
    Ok(())
}

#[test]
fn duplicate_session_identifiers_are_rejected_without_commit() -> TestResult {
    let (manager, store) = setup(2)?;
    let _ = create(&manager, "session-a", 0, 10)?;
    let before = store.snapshot()?;
    let result = manager.create_session(create_command("session-a", 1, 11)?);

    assert!(matches!(
        result,
        Err(SessionManagerError::DuplicateSession { ref session_id })
            if session_id.as_str() == "session-a"
    ));
    assert_eq!(store.snapshot()?, before);
    Ok(())
}

#[test]
fn invalid_session_identifiers_are_rejected_by_the_strong_type() {
    assert!(matches!(
        SessionId::new(""),
        Err(ref error) if error.kind() == StateIdentifierKind::Session
    ));
}

#[test]
fn creation_rejects_missing_devices_and_connectors() -> TestResult {
    let (manager, store) = setup(1)?;
    let missing_device = manager.create_session(CreateSession::new(
        session_id("missing-device")?,
        DeviceId::new("absent-device")?,
        connector(0)?,
        capabilities(),
        protocol_version(),
        timestamp(10),
    ));
    assert!(matches!(
        missing_device,
        Err(SessionManagerError::MissingDevice { ref device_id })
            if device_id.as_str() == "absent-device"
    ));

    let missing_connector = manager.create_session(CreateSession::new(
        session_id("missing-connector")?,
        device(0)?,
        ConnectorId::new("absent-connector")?,
        capabilities(),
        protocol_version(),
        timestamp(10),
    ));
    assert!(matches!(
        missing_connector,
        Err(SessionManagerError::MissingConnector { ref connector_id })
            if connector_id.as_str() == "absent-connector"
    ));
    assert_eq!(store.snapshot()?.revision().get(), 1);
    Ok(())
}

#[test]
fn duplicate_live_associations_are_rejected() -> TestResult {
    let (manager, store) = setup(1)?;
    let _ = create(&manager, "session-a", 0, 10)?;
    let result = manager.create_session(create_command("session-b", 0, 11)?);

    assert!(matches!(
        result,
        Err(SessionManagerError::DuplicateLiveAssociation {
            ref session_id,
            ref conflicting_session_id,
            ..
        }) if session_id.as_str() == "session-b"
            && conflicting_session_id.as_str() == "session-a"
    ));
    assert_eq!(store.snapshot()?.sessions().len(), 1);
    Ok(())
}

#[test]
fn closed_sessions_release_unique_associations() -> TestResult {
    let (manager, _) = setup(1)?;
    let _ = manager.restore_session(restore_command(
        "closed-session",
        0,
        SessionLifecycleState::Closed,
        4,
    )?)?;
    let created = create(&manager, "replacement-session", 0, 10)?;

    assert_eq!(created.session().lifecycle(), SessionLifecycleState::Created);
    Ok(())
}

#[test]
fn valid_session_restore_preserves_persisted_fields() -> TestResult {
    let (manager, _) = setup(1)?;
    let result = manager.restore_session(restore_command(
        "restored",
        0,
        SessionLifecycleState::Suspended,
        7,
    )?)?;

    assert_eq!(result.session().lifecycle(), SessionLifecycleState::Suspended);
    assert_eq!(result.session().revision(), SessionRevision::new(7));
    assert_eq!(result.session().created_at(), timestamp(1));
    assert_eq!(result.session().last_activity_at(), timestamp(2));
    let event = transition(result.state_update())?;
    assert_eq!(event.previous(), None);
    assert_eq!(event.current(), Some(SessionLifecycleState::Suspended));
    assert_eq!(event.timestamp(), timestamp(3));
    Ok(())
}

#[test]
fn restore_rejects_invalid_timestamp_order_and_expiration() -> TestResult {
    let (manager, store) = setup(1)?;
    let invalid = RestoreSession::new(
        session_id("invalid-time")?,
        device(0)?,
        connector(0)?,
        capabilities(),
        protocol_version(),
        SessionLifecycleState::Active,
        SessionRevision::new(1),
        timestamp(5),
        timestamp(4),
        timestamp(6),
    );
    assert!(matches!(
        manager.restore_session(invalid),
        Err(SessionManagerError::InvalidRestoreTimestamps { .. })
    ));

    let expired = RestoreSession::new(
        session_id("expired")?,
        device(0)?,
        connector(0)?,
        capabilities(),
        protocol_version(),
        SessionLifecycleState::Active,
        SessionRevision::new(1),
        timestamp(1),
        timestamp(2),
        timestamp(102),
    );
    assert!(matches!(
        manager.restore_session(expired),
        Err(SessionManagerError::ExpiredSession { .. })
    ));
    assert_eq!(store.snapshot()?.sessions().len(), 0);
    Ok(())
}

#[test]
fn every_allowed_lifecycle_transition_succeeds() -> TestResult {
    let allowed = [
        (SessionLifecycleState::Created, SessionLifecycleState::Negotiating),
        (SessionLifecycleState::Created, SessionLifecycleState::Closing),
        (SessionLifecycleState::Negotiating, SessionLifecycleState::Active),
        (SessionLifecycleState::Negotiating, SessionLifecycleState::Closing),
        (SessionLifecycleState::Active, SessionLifecycleState::Suspended),
        (SessionLifecycleState::Active, SessionLifecycleState::Closing),
        (SessionLifecycleState::Suspended, SessionLifecycleState::Active),
        (SessionLifecycleState::Suspended, SessionLifecycleState::Closing),
        (SessionLifecycleState::Closing, SessionLifecycleState::Closed),
    ];

    for (index, (previous, current)) in allowed.into_iter().enumerate() {
        let (manager, _) = setup(1)?;
        let identifier = format!("allowed-{index}");
        let restored = manager.restore_session(restore_command(
            identifier.clone(),
            0,
            previous,
            4,
        )?)?;
        let updated = manager.update_session(
            UpdateSession::new(
                session_id(identifier)?,
                restored.session().revision(),
                timestamp(4),
            )
            .with_lifecycle(current),
        )?;
        assert_eq!(updated.session().lifecycle(), current);
        assert_eq!(transition(updated.state_update())?.previous(), Some(previous));
        assert_eq!(transition(updated.state_update())?.current(), Some(current));
    }
    Ok(())
}

#[test]
fn every_disallowed_lifecycle_transition_is_rejected() -> TestResult {
    let states = [
        SessionLifecycleState::Created,
        SessionLifecycleState::Negotiating,
        SessionLifecycleState::Active,
        SessionLifecycleState::Suspended,
        SessionLifecycleState::Closing,
        SessionLifecycleState::Closed,
    ];

    for previous in states {
        for requested in states {
            if previous.can_transition_to(requested) {
                continue;
            }
            let (manager, store) = setup(1)?;
            let identifier = format!("invalid-{previous:?}-{requested:?}");
            let restored = manager.restore_session(restore_command(
                identifier.clone(),
                0,
                previous,
                2,
            )?)?;
            let before = store.snapshot()?;
            let result = manager.update_session(
                UpdateSession::new(
                    session_id(identifier)?,
                    restored.session().revision(),
                    timestamp(4),
                )
                .with_lifecycle(requested),
            );
            if previous.is_terminal() {
                assert!(matches!(result, Err(SessionManagerError::TerminalSession { .. })));
            } else {
                assert!(matches!(result, Err(SessionManagerError::InvalidTransition { .. })));
            }
            assert_eq!(store.snapshot()?, before);
        }
    }
    Ok(())
}

#[test]
fn suspend_resume_and_close_apis_follow_the_state_machine() -> TestResult {
    let (manager, _) = setup(1)?;
    let restored = manager.restore_session(restore_command(
        "lifecycle",
        0,
        SessionLifecycleState::Active,
        5,
    )?)?;
    let suspended = manager.suspend_session(SuspendSession::new(
        session_id("lifecycle")?,
        restored.session().revision(),
        timestamp(4),
    ))?;
    assert_eq!(suspended.session().lifecycle(), SessionLifecycleState::Suspended);

    let resumed = manager.resume_session(ResumeSession::new(
        session_id("lifecycle")?,
        suspended.session().revision(),
        timestamp(5),
    ))?;
    assert_eq!(resumed.session().lifecycle(), SessionLifecycleState::Active);

    let closing = manager.close_session(CloseSession::new(
        session_id("lifecycle")?,
        resumed.session().revision(),
        timestamp(6),
    ))?;
    assert_eq!(closing.session().lifecycle(), SessionLifecycleState::Closing);

    let closed = manager.close_session(CloseSession::new(
        session_id("lifecycle")?,
        closing.session().revision(),
        timestamp(7),
    ))?;
    assert_eq!(closed.session().lifecycle(), SessionLifecycleState::Closed);
    assert!(matches!(
        manager.close_session(CloseSession::new(
            session_id("lifecycle")?,
            closed.session().revision(),
            timestamp(8),
        )),
        Err(SessionManagerError::TerminalSession { .. })
    ));
    Ok(())
}

#[test]
fn repeated_suspend_and_resume_transitions_are_rejected() -> TestResult {
    let (manager, _) = setup(1)?;
    let restored = manager.restore_session(restore_command(
        "repeat",
        0,
        SessionLifecycleState::Active,
        1,
    )?)?;
    let suspended = manager.suspend_session(SuspendSession::new(
        session_id("repeat")?,
        restored.session().revision(),
        timestamp(4),
    ))?;
    assert!(matches!(
        manager.suspend_session(SuspendSession::new(
            session_id("repeat")?,
            suspended.session().revision(),
            timestamp(5),
        )),
        Err(SessionManagerError::InvalidTransition { .. })
    ));
    assert!(matches!(
        manager.resume_session(ResumeSession::new(
            session_id("repeat")?,
            SessionRevision::new(999),
            timestamp(5),
        )),
        Err(SessionManagerError::StaleRevision { .. })
    ));
    Ok(())
}

#[test]
fn update_replaces_metadata_capabilities_and_protocol_version() -> TestResult {
    let (manager, _) = setup(1)?;
    let created = create(&manager, "update", 0, 10)?;
    let mut metadata = SessionMetadata::new();
    let key = SessionMetadataKey::new("locale")?;
    let _ = metadata.insert(key.clone(), SessionMetadataValue::new("en-US"));
    let replacement_capabilities = CapabilitySet {
        supported: vec![Capability::CapabilityPause as i32],
        ..CapabilitySet::default()
    };
    let replacement_version = ProtocolVersion {
        major: 2,
        minor: 0,
        patch: 0,
    };

    let updated = manager.update_session(
        UpdateSession::new(
            session_id("update")?,
            created.session().revision(),
            timestamp(11),
        )
        .with_metadata(metadata)
        .with_capabilities(replacement_capabilities.clone())
        .with_protocol_version(replacement_version.clone()),
    )?;

    assert_eq!(updated.session().revision(), SessionRevision::new(1));
    assert_eq!(updated.session().last_activity_at(), timestamp(11));
    assert_eq!(updated.session().capabilities(), &replacement_capabilities);
    assert_eq!(updated.session().protocol_version(), &replacement_version);
    assert_eq!(
        updated.session().metadata().get(&key).map(SessionMetadataValue::as_str),
        Some("en-US")
    );
    assert!(transition(updated.state_update()).is_err());
    Ok(())
}

#[test]
fn identical_update_is_idempotent() -> TestResult {
    let (manager, store) = setup(1)?;
    let created = create(&manager, "idempotent", 0, 10)?;
    let before = store.snapshot()?;
    let updated = manager.update_session(UpdateSession::new(
        session_id("idempotent")?,
        created.session().revision(),
        timestamp(10),
    ))?;

    assert!(!updated.state_update().changed_state());
    assert_eq!(updated.session().revision(), SessionRevision::INITIAL);
    assert_eq!(store.snapshot()?, before);
    Ok(())
}

#[test]
fn lookup_list_and_exists_use_deterministic_snapshots() -> TestResult {
    let (manager, _) = setup(3)?;
    let _ = create(&manager, "session-z", 0, 10)?;
    let _ = create(&manager, "session-a", 1, 10)?;
    let _ = create(&manager, "session-m", 2, 10)?;

    assert!(manager.session_exists(&session_id("session-a")?)?);
    assert!(!manager.session_exists(&session_id("missing")?)?);
    assert_eq!(
        manager
            .lookup_session(&session_id("session-m")?)?
            .map(|session| session.session_id().as_str().to_owned()),
        Some("session-m".to_owned())
    );
    assert!(manager.lookup_session(&session_id("missing")?)?.is_none());
    assert_eq!(
        manager
            .list_sessions()?
            .iter()
            .map(|session| session.session_id().as_str())
            .collect::<Vec<_>>(),
        vec!["session-a", "session-m", "session-z"]
    );
    Ok(())
}

#[test]
fn stale_revisions_timestamp_regressions_and_expiration_are_structured() -> TestResult {
    let (manager, store) = setup(1)?;
    let created = create(&manager, "validation", 0, 10)?;
    let before = store.snapshot()?;

    assert!(matches!(
        manager.update_session(UpdateSession::new(
            session_id("validation")?,
            SessionRevision::new(1),
            timestamp(11),
        )),
        Err(SessionManagerError::StaleRevision { .. })
    ));
    assert!(matches!(
        manager.update_session(UpdateSession::new(
            session_id("validation")?,
            created.session().revision(),
            timestamp(9),
        )),
        Err(SessionManagerError::TimestampRegression { .. })
    ));
    assert!(matches!(
        manager.update_session(UpdateSession::new(
            session_id("validation")?,
            created.session().revision(),
            timestamp(110),
        )),
        Err(SessionManagerError::ExpiredSession { .. })
    ));
    assert_eq!(store.snapshot()?, before);
    Ok(())
}

#[test]
fn missing_session_operations_do_not_commit() -> TestResult {
    let (manager, store) = setup(1)?;
    let before = store.snapshot()?;
    let result = manager.update_session(UpdateSession::new(
        session_id("missing")?,
        SessionRevision::INITIAL,
        timestamp(10),
    ));

    assert!(matches!(result, Err(SessionManagerError::SessionNotFound { .. })));
    assert_eq!(store.snapshot()?, before);
    Ok(())
}

#[test]
fn invalid_protocol_and_capability_values_are_rejected() -> TestResult {
    let (manager, store) = setup(3)?;
    let invalid_version = manager.create_session(CreateSession::new(
        session_id("invalid-version")?,
        device(0)?,
        connector(0)?,
        capabilities(),
        ProtocolVersion::default(),
        timestamp(10),
    ));
    assert!(matches!(
        invalid_version,
        Err(SessionManagerError::InvalidProtocolVersion { .. })
    ));

    let duplicate = manager.create_session(CreateSession::new(
        session_id("duplicate-capability")?,
        device(1)?,
        connector(1)?,
        CapabilitySet {
            supported: vec![
                Capability::CapabilityPlay as i32,
                Capability::CapabilityPlay as i32,
            ],
            ..CapabilitySet::default()
        },
        protocol_version(),
        timestamp(10),
    ));
    assert!(matches!(
        duplicate,
        Err(SessionManagerError::DuplicateCapability { .. })
    ));

    let missing_required = manager.create_session(CreateSession::new(
        session_id("missing-required")?,
        device(2)?,
        connector(2)?,
        CapabilitySet {
            required: vec![Capability::CapabilityPlay as i32],
            ..CapabilitySet::default()
        },
        protocol_version(),
        timestamp(10),
    ));
    assert!(matches!(
        missing_required,
        Err(SessionManagerError::MissingRequiredCapability { .. })
    ));
    assert_eq!(store.snapshot()?.sessions().len(), 0);
    Ok(())
}

#[test]
fn expiration_removes_only_timed_out_sessions() -> TestResult {
    let (manager, store) = setup(3)?;
    let _ = create(&manager, "expired-a", 0, 1)?;
    let _ = create(&manager, "active", 1, 50)?;
    let _ = create(&manager, "expired-b", 2, 2)?;

    let result = manager.remove_expired_sessions(RemoveExpiredSessions::new(timestamp(101)))?;
    assert_eq!(
        result
            .removed()
            .iter()
            .map(SessionId::as_str)
            .collect::<Vec<_>>(),
        vec!["expired-a"]
    );
    assert!(manager.session_exists(&session_id("active")?)?);
    assert!(manager.session_exists(&session_id("expired-b")?)?);
    assert_eq!(store.snapshot()?.sessions().len(), 2);
    Ok(())
}

#[test]
fn expiration_order_and_events_are_deterministic() -> TestResult {
    let (manager, _) = setup(3)?;
    let _ = create(&manager, "z-expired", 0, 1)?;
    let _ = create(&manager, "a-expired", 1, 1)?;
    let _ = create(&manager, "m-expired", 2, 1)?;
    let result = manager.remove_expired_sessions(RemoveExpiredSessions::new(timestamp(101)))?;

    assert_eq!(
        result
            .removed()
            .iter()
            .map(SessionId::as_str)
            .collect::<Vec<_>>(),
        vec!["a-expired", "m-expired", "z-expired"]
    );
    let event = result
        .state_update()
        .event()
        .ok_or_else(|| io::Error::other("missing expiration event"))?;
    let transitions = event
        .changes()
        .iter()
        .filter_map(|change| match change {
            BridgeStateChange::SessionLifecycle(transition) => Some(transition),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        transitions
            .iter()
            .map(|transition| transition.session_id().as_str())
            .collect::<Vec<_>>(),
        vec!["a-expired", "m-expired", "z-expired"]
    );
    assert!(transitions.iter().all(|transition| {
        transition.current().is_none() && transition.timestamp() == timestamp(101)
    }));
    Ok(())
}

#[test]
fn expiration_noop_does_not_increment_state_revision() -> TestResult {
    let (manager, store) = setup(1)?;
    let _ = create(&manager, "active", 0, 10)?;
    let before = store.snapshot()?;
    let result = manager.remove_expired_sessions(RemoveExpiredSessions::new(timestamp(20)))?;

    assert!(result.removed().is_empty());
    assert!(!result.state_update().changed_state());
    assert_eq!(store.snapshot()?, before);
    Ok(())
}

#[test]
fn lifecycle_events_contain_required_typed_fields() -> TestResult {
    let (manager, _) = setup(1)?;
    let created = create(&manager, "events", 0, 10)?;
    let creation = transition(created.state_update())?;
    assert_eq!(creation.session_id(), &session_id("events")?);
    assert_eq!(creation.previous(), None);
    assert_eq!(creation.current(), Some(SessionLifecycleState::Created));
    assert_eq!(creation.session_revision(), SessionRevision::INITIAL);
    assert_eq!(creation.timestamp(), timestamp(10));
    assert_eq!(created.state_update().event().map(BridgeStateEvent::revision), Some(StateRevision::new(2)));

    let negotiating = manager.update_session(
        UpdateSession::new(
            session_id("events")?,
            created.session().revision(),
            timestamp(11),
        )
        .with_lifecycle(SessionLifecycleState::Negotiating),
    )?;
    let lifecycle = transition(negotiating.state_update())?;
    assert_eq!(lifecycle.previous(), Some(SessionLifecycleState::Created));
    assert_eq!(lifecycle.current(), Some(SessionLifecycleState::Negotiating));
    assert_eq!(lifecycle.session_revision(), SessionRevision::new(1));
    assert_eq!(lifecycle.timestamp(), timestamp(11));
    Ok(())
}

#[test]
fn rejected_transitions_emit_no_event_and_do_not_partially_update() -> TestResult {
    let (manager, store) = setup(1)?;
    let created = create(&manager, "rollback", 0, 10)?;
    let subscription = store.subscribe()?;
    let mut metadata = SessionMetadata::new();
    let key = SessionMetadataKey::new("should-not-commit")?;
    let _ = metadata.insert(key.clone(), SessionMetadataValue::new("value"));
    let before = store.snapshot()?;

    let result = manager.update_session(
        UpdateSession::new(
            session_id("rollback")?,
            created.session().revision(),
            timestamp(11),
        )
        .with_lifecycle(SessionLifecycleState::Active)
        .with_metadata(metadata),
    );
    assert!(matches!(result, Err(SessionManagerError::InvalidTransition { .. })));
    assert_eq!(subscription.try_recv(), Err(StateReceiveError::Empty));
    assert_eq!(store.snapshot()?, before);
    assert!(manager
        .lookup_session(&session_id("rollback")?)?
        .and_then(|session| session.metadata().get(&key).cloned())
        .is_none());
    Ok(())
}

#[test]
fn state_snapshots_remain_immutable_across_session_updates() -> TestResult {
    let (manager, store) = setup(1)?;
    let created = create(&manager, "snapshot", 0, 10)?;
    let before = store.snapshot()?;
    let _ = manager.update_session(
        UpdateSession::new(
            session_id("snapshot")?,
            created.session().revision(),
            timestamp(11),
        )
        .with_lifecycle(SessionLifecycleState::Negotiating),
    )?;

    assert_eq!(
        before.sessions().get(&session_id("snapshot")?).map(BridgeSession::lifecycle),
        Some(SessionLifecycleState::Created)
    );
    assert_eq!(
        store
            .snapshot()?
            .sessions()
            .get(&session_id("snapshot")?)
            .map(BridgeSession::lifecycle),
        Some(SessionLifecycleState::Negotiating)
    );
    Ok(())
}

#[test]
fn concurrent_session_creation_is_serialized() -> TestResult {
    let (manager, store) = setup(8)?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();

    for index in 0..8 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), SessionManagerError> {
            barrier.wait();
            let command = CreateSession::new(
                SessionId::new(format!("concurrent-{index}"))
                    .map_err(|error| SessionManagerError::state_invariant(error.to_string()))?,
                DeviceId::new(format!("device-{index:02}"))
                    .map_err(|error| SessionManagerError::state_invariant(error.to_string()))?,
                ConnectorId::new(format!("connector-{index:02}"))
                    .map_err(|error| SessionManagerError::state_invariant(error.to_string()))?,
                capabilities(),
                protocol_version(),
                timestamp(10),
            );
            let _ = manager.create_session(command)?;
            Ok(())
        }));
    }

    barrier.wait();
    for worker in workers {
        join(worker)??;
    }
    assert_eq!(store.snapshot()?.sessions().len(), 8);
    assert_eq!(store.snapshot()?.revision().get(), 9);
    Ok(())
}

#[test]
fn concurrent_lookups_are_consistent() -> TestResult {
    let (manager, _) = setup(1)?;
    let _ = create(&manager, "lookup", 0, 10)?;
    let manager = Arc::new(manager);
    let mut workers = Vec::new();

    for _ in 0..8 {
        let manager = Arc::clone(&manager);
        workers.push(thread::spawn(move || -> Result<(), SessionManagerError> {
            for _ in 0..100 {
                let identifier = SessionId::new("lookup")
                    .map_err(|error| SessionManagerError::state_invariant(error.to_string()))?;
                let session = manager.lookup_session(&identifier)?.ok_or_else(|| {
                    SessionManagerError::state_invariant("concurrent lookup lost session")
                })?;
                if session.revision() != SessionRevision::INITIAL {
                    return Err(SessionManagerError::state_invariant(
                        "concurrent lookup observed unexpected revision",
                    ));
                }
            }
            Ok(())
        }));
    }
    for worker in workers {
        join(worker)??;
    }
    Ok(())
}

#[test]
fn concurrent_close_uses_optimistic_revision_consistency() -> TestResult {
    let (manager, store) = setup(1)?;
    let restored = manager.restore_session(restore_command(
        "concurrent-close",
        0,
        SessionLifecycleState::Active,
        5,
    )?)?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        let revision = restored.session().revision();
        workers.push(thread::spawn(move || {
            barrier.wait();
            let identifier = SessionId::new("concurrent-close");
            match identifier {
                Ok(identifier) => manager.close_session(CloseSession::new(
                    identifier,
                    revision,
                    timestamp(4),
                )),
                Err(error) => Err(SessionManagerError::state_invariant(error.to_string())),
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
            .filter(|result| matches!(result, Err(SessionManagerError::StaleRevision { .. })))
            .count(),
        1
    );
    assert_eq!(
        store
            .snapshot()?
            .sessions()
            .get(&session_id("concurrent-close")?)
            .map(BridgeSession::lifecycle),
        Some(SessionLifecycleState::Closing)
    );
    Ok(())
}

#[test]
fn identical_operations_produce_deterministic_snapshots_and_events() -> TestResult {
    let (first, first_store) = setup(1)?;
    let (second, second_store) = setup(1)?;
    let first_created = create(&first, "deterministic", 0, 10)?;
    let second_created = create(&second, "deterministic", 0, 10)?;
    let first_updated = first.update_session(
        UpdateSession::new(
            session_id("deterministic")?,
            first_created.session().revision(),
            timestamp(11),
        )
        .with_lifecycle(SessionLifecycleState::Negotiating),
    )?;
    let second_updated = second.update_session(
        UpdateSession::new(
            session_id("deterministic")?,
            second_created.session().revision(),
            timestamp(11),
        )
        .with_lifecycle(SessionLifecycleState::Negotiating),
    )?;

    assert_eq!(first_store.snapshot()?, second_store.snapshot()?);
    assert_eq!(first_updated.state_update().event(), second_updated.state_update().event());
    Ok(())
}

#[test]
fn metadata_container_and_policy_are_strongly_typed() -> TestResult {
    assert_eq!(SessionDuration::from_millis(0), Err(SessionModelError::ZeroDuration));
    assert_eq!(SessionMetadataKey::new(""), Err(SessionModelError::EmptyMetadataKey));

    let timeout = SessionDuration::from_millis(250)?;
    let policy = SessionPolicy::new(timeout).with_unique_live_association(false);
    assert_eq!(policy.inactivity_timeout(), timeout);
    assert!(!policy.enforces_unique_live_association());

    let mut metadata = SessionMetadata::new();
    let first = SessionMetadataKey::new("z-key")?;
    let second = SessionMetadataKey::new("a-key")?;
    let _ = metadata.insert(first, SessionMetadataValue::new("z"));
    let _ = metadata.insert(second, SessionMetadataValue::new("a"));
    assert_eq!(
        metadata
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["a-key", "z-key"]
    );
    Ok(())
}

#[test]
fn session_manager_and_records_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SessionManager>();
    assert_send_sync::<BridgeSession>();
    assert_send_sync::<SessionMutation>();
    assert_send_sync::<ExpiredSessions>();
}
