use std::{
    error::Error,
    io,
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Barrier,
    },
    thread,
    time::Duration,
};

use ym_connect_protocol::v1::{
    BrowserDescriptor, CapabilitySet, DeviceDescriptor, ProtocolVersion,
};

use crate::{
    session::SessionRecordParts, state::*, BridgeConfig, BridgeConfigLayer, BridgeSession,
    LogLevel, SessionLifecycleState, SessionMetadata, SessionRevision, SessionTimestamp,
};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn debug_configuration() -> TestResult<BridgeConfig> {
    Ok(BridgeConfig::loader()
        .without_environment()
        .with_layer(BridgeConfigLayer::new().with_log_level(LogLevel::Debug))
        .load()?)
}

fn device(identifier: impl Into<String>, display_name: impl Into<String>) -> DeviceDescriptor {
    DeviceDescriptor {
        device_id: identifier.into(),
        display_name: display_name.into(),
        ..DeviceDescriptor::default()
    }
}

fn session(identifier: impl Into<String>) -> Result<BridgeSession, StateError> {
    let session_id = SessionId::new(identifier.into())
        .map_err(|error| StateError::rejected("state-test-session-id", error.to_string()))?;
    let device_id = DeviceId::new("state-test-device")
        .map_err(|error| StateError::rejected("state-test-device-id", error.to_string()))?;
    let connector_id = ConnectorId::new("state-test-connector")
        .map_err(|error| StateError::rejected("state-test-connector-id", error.to_string()))?;

    Ok(BridgeSession::from_parts(SessionRecordParts {
        session_id,
        created_at: SessionTimestamp::from_unix_millis(1),
        last_activity_at: SessionTimestamp::from_unix_millis(1),
        lifecycle: SessionLifecycleState::Created,
        device_id,
        connector_id,
        capabilities: CapabilitySet::default(),
        protocol_version: ProtocolVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        revision: SessionRevision::INITIAL,
        metadata: SessionMetadata::new(),
    }))
}

fn connector(identifier: impl Into<String>) -> BrowserDescriptor {
    BrowserDescriptor {
        connector_id: identifier.into(),
        ..BrowserDescriptor::default()
    }
}

fn join<T>(handle: thread::JoinHandle<T>) -> TestResult<T> {
    handle
        .join()
        .map_err(|_| io::Error::other("test worker panicked").into())
}

#[test]
fn store_creation_initializes_complete_default_state() -> TestResult {
    let store = BridgeStateStore::default();
    let snapshot = store.snapshot()?;

    assert_eq!(snapshot.revision(), StateRevision::INITIAL);
    assert_eq!(snapshot.lifecycle(), BridgeLifecycleState::Initializing);
    assert_eq!(snapshot.configuration(), &BridgeConfig::default());
    assert!(snapshot.sessions().is_empty());
    assert!(snapshot.devices().is_empty());
    assert!(snapshot.connectors().is_empty());
    assert!(snapshot.capabilities().is_empty());
    Ok(())
}

#[test]
fn snapshots_remain_immutable_after_later_updates() -> TestResult {
    let store = BridgeStateStore::default();
    let before = store.snapshot()?;

    let update = store.update(|draft| {
        let _ = draft.devices_mut().insert(device("device-a", "First"))?;
        Ok(())
    })?;

    assert!(before.devices().is_empty());
    assert_eq!(before.revision(), StateRevision::INITIAL);
    assert_eq!(update.snapshot().devices().len(), 1);
    assert_eq!(update.snapshot().revision().get(), 1);
    Ok(())
}

#[test]
fn concurrent_reads_observe_consistent_snapshots() -> TestResult {
    let store = Arc::new(BridgeStateStore::default());
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();

    for _ in 0..8 {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), StateError> {
            barrier.wait();
            for _ in 0..200 {
                let snapshot = store.snapshot()?;
                if snapshot.revision() != StateRevision::INITIAL || !snapshot.devices().is_empty() {
                    return Err(StateError::rejected(
                        "inconsistent-read",
                        "default snapshot changed during read-only stress test",
                    ));
                }
            }
            Ok(())
        }));
    }

    barrier.wait();
    for worker in workers {
        join(worker)??;
    }
    Ok(())
}

#[test]
fn concurrent_writes_are_serialized_and_increment_revisions() -> TestResult {
    let store = Arc::new(BridgeStateStore::default());
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();

    for worker_index in 0..8 {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), StateError> {
            barrier.wait();
            for item_index in 0..50 {
                let identifier = format!("device-{worker_index:02}-{item_index:03}");
                store.update(|draft| {
                    let _ = draft
                        .devices_mut()
                        .insert(device(identifier, "Concurrent device"))?;
                    Ok(())
                })?;
            }
            Ok(())
        }));
    }

    barrier.wait();
    for worker in workers {
        join(worker)??;
    }

    let snapshot = store.snapshot()?;
    assert_eq!(snapshot.devices().len(), 400);
    assert_eq!(snapshot.revision().get(), 400);
    Ok(())
}

#[test]
fn revision_changes_only_when_state_changes() -> TestResult {
    let store = BridgeStateStore::default();
    let first = store.update(|draft| {
        let _ = draft.devices_mut().upsert(device("device-a", "Name"))?;
        Ok(())
    })?;
    let expected_key = DeviceId::new("device-a")?;
    let second = store.update(|draft| {
        let mutation = draft.devices_mut().upsert(device("device-a", "Name"))?;
        assert_eq!(mutation, RegistryMutation::Unchanged(expected_key));
        Ok(())
    })?;

    assert!(first.changed_state());
    assert!(!second.changed_state());
    assert_eq!(first.snapshot().revision(), second.snapshot().revision());
    Ok(())
}

#[test]
fn identical_input_produces_identical_deterministic_snapshots() -> TestResult {
    let first = BridgeStateStore::default();
    let second = BridgeStateStore::default();

    for store in [&first, &second] {
        store.update(|draft| {
            let _ = draft.devices_mut().insert(device("z-device", "Z"))?;
            let _ = draft.devices_mut().insert(device("a-device", "A"))?;
            let _ = draft.sessions_mut().insert(session("session-b")?)?;
            let _ = draft.sessions_mut().insert(session("session-a")?)?;
            Ok(())
        })?;
    }

    let first_snapshot = first.snapshot()?;
    let second_snapshot = second.snapshot()?;
    assert_eq!(first_snapshot, second_snapshot);
    assert_eq!(
        first_snapshot
            .devices()
            .keys()
            .map(DeviceId::as_str)
            .collect::<Vec<_>>(),
        vec!["a-device", "z-device"]
    );
    assert_eq!(
        first_snapshot
            .sessions()
            .keys()
            .map(SessionId::as_str)
            .collect::<Vec<_>>(),
        vec!["session-a", "session-b"]
    );
    Ok(())
}

#[test]
fn registries_support_insert_lookup_iteration_and_empty_state() -> TestResult {
    let mut devices = DeviceRegistry::new();
    assert!(devices.is_empty());

    let mutation = devices.insert(device("device-b", "B"))?;
    assert_eq!(
        mutation,
        RegistryMutation::Inserted(DeviceId::new("device-b")?)
    );
    let _ = devices.insert(device("device-a", "A"))?;

    let key = DeviceId::new("device-a")?;
    assert!(devices.contains_key(&key));
    assert_eq!(
        devices
            .get(&key)
            .map(|entry| entry.display_name.as_str()),
        Some("A")
    );
    assert_eq!(
        devices.keys().map(DeviceId::as_str).collect::<Vec<_>>(),
        vec!["device-a", "device-b"]
    );
    assert_eq!(devices.values().count(), 2);
    Ok(())
}

#[test]
fn registry_replacement_and_removal_are_explicit() -> TestResult {
    let mut devices = DeviceRegistry::new();
    let key = DeviceId::new("device-a")?;
    let _ = devices.insert(device("device-a", "Old"))?;

    let replacement = devices.replace(device("device-a", "New"))?;
    assert_eq!(replacement, RegistryMutation::Replaced(key.clone()));
    assert_eq!(
        devices
            .get(&key)
            .map(|entry| entry.display_name.as_str()),
        Some("New")
    );

    let (removal, removed) = devices.remove(&key)?;
    assert_eq!(removal, RegistryMutation::Removed(key));
    assert_eq!(removed.display_name.as_str(), "New");
    assert!(devices.is_empty());
    Ok(())
}

#[test]
fn registry_duplicate_and_missing_operations_return_structured_errors() -> TestResult {
    let mut devices = DeviceRegistry::new();
    let key = DeviceId::new("device-a")?;
    let _ = devices.insert(device("device-a", "A"))?;

    let duplicate = devices.insert(device("device-a", "Duplicate"));
    assert!(matches!(
        duplicate,
        Err(ref error)
            if error.registry() == RegistryKind::Devices
                && error.operation() == RegistryOperation::Insert
                && error.failure() == RegistryFailure::DuplicateKey
                && error.key() == Some("device-a")
    ));

    let missing = devices.remove(&DeviceId::new("missing")?);
    assert!(matches!(
        missing,
        Err(ref error)
            if error.operation() == RegistryOperation::Remove
                && error.failure() == RegistryFailure::MissingKey
    ));

    assert!(devices.remove_if_present(&key).is_some());
    assert!(devices.remove_if_present(&key).is_none());
    Ok(())
}

#[test]
fn all_concrete_registries_accept_canonical_records() -> TestResult {
    let mut sessions = SessionRegistry::new();
    let mut connectors = ConnectorRegistry::new();
    let mut capabilities = CapabilityRegistry::new();

    let _ = sessions.insert(session("session-a")?)?;
    let _ = connectors.insert(connector("connector-a"))?;
    let _ = capabilities.insert(CapabilityRegistration::new(
        CapabilityOwner::Bridge,
        CapabilitySet {
            supported: vec![1, 2],
            ..CapabilitySet::default()
        },
    ))?;

    assert_eq!(sessions.len(), 1);
    assert_eq!(connectors.len(), 1);
    assert_eq!(capabilities.len(), 1);
    assert!(capabilities.get(&CapabilityOwner::Bridge).is_some());
    Ok(())
}

#[test]
fn invalid_canonical_identifiers_are_rejected() {
    let mut devices = DeviceRegistry::new();
    let result = devices.insert(device("", "Invalid"));

    assert!(matches!(
        result,
        Err(ref error)
            if error.registry() == RegistryKind::Devices
                && error.failure() == RegistryFailure::InvalidIdentifier
    ));
}

#[test]
fn clear_reports_removed_entries_and_is_idempotent() -> TestResult {
    let mut devices = DeviceRegistry::new();
    let _ = devices.insert(device("device-a", "A"))?;
    let _ = devices.insert(device("device-b", "B"))?;

    assert_eq!(devices.clear(), 2);
    assert_eq!(devices.clear(), 0);
    assert!(devices.is_empty());
    Ok(())
}

#[test]
fn subscription_captures_initial_snapshot_without_race() -> TestResult {
    let store = BridgeStateStore::default();
    let _ = store.update(|draft| {
        let _ = draft.devices_mut().insert(device("device-a", "A"))?;
        Ok(())
    })?;
    let subscription = store.subscribe()?;

    assert_eq!(subscription.initial_snapshot(), &store.snapshot()?);
    assert_eq!(store.subscriber_count()?, 1);
    Ok(())
}

#[test]
fn subscribers_receive_notifications_in_revision_order() -> TestResult {
    let store = BridgeStateStore::default();
    let subscription = store.subscribe()?;

    for index in 0..20 {
        let identifier = format!("device-{index:02}");
        let _ = store.update(|draft| {
            let _ = draft
                .devices_mut()
                .insert(device(identifier, "Ordered"))?;
            Ok(())
        })?;
    }

    for expected_revision in 1..=20 {
        let event = subscription.recv_timeout(Duration::from_secs(1))?;
        assert_eq!(event.revision().get(), expected_revision);
        assert_eq!(event.previous_revision().get(), expected_revision - 1);
    }
    Ok(())
}

#[test]
fn multiple_subscribers_receive_the_same_event() -> TestResult {
    let store = BridgeStateStore::default();
    let first = store.subscribe()?;
    let second = store.subscribe()?;

    let update = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        Ok(())
    })?;
    let first_event = first.recv_timeout(Duration::from_secs(1))?;
    let second_event = second.recv_timeout(Duration::from_secs(1))?;

    assert_eq!(first_event, second_event);
    assert_eq!(update.notifications().attempted(), 2);
    assert_eq!(update.notifications().delivered(), 2);
    Ok(())
}

#[test]
fn unsubscribe_prevents_future_notifications() -> TestResult {
    let store = BridgeStateStore::default();
    let subscription = store.subscribe()?;

    assert!(subscription.unsubscribe()?);
    assert!(!subscription.unsubscribe()?);
    let update = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        Ok(())
    })?;

    assert_eq!(update.notifications().attempted(), 0);
    assert_eq!(subscription.try_recv(), Err(StateReceiveError::Disconnected));
    Ok(())
}

#[test]
fn disconnected_subscribers_do_not_block_other_subscribers() -> TestResult {
    let store = BridgeStateStore::default();
    let disconnected = store.subscribe()?;
    let active = store.subscribe()?;
    drop(disconnected);

    let update = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        Ok(())
    })?;

    assert_eq!(
        active.recv_timeout(Duration::from_secs(1))?.revision().get(),
        1
    );
    assert_eq!(update.notifications().delivered(), 1);
    assert_eq!(store.subscriber_count()?, 1);
    Ok(())
}

#[test]
fn subscriber_panics_are_isolated_by_message_passing() -> TestResult {
    let store = BridgeStateStore::default();
    let subscription = store.subscribe()?;
    let worker = thread::spawn(move || {
        let _ = subscription.recv_timeout(Duration::from_secs(1));
        panic::resume_unwind(Box::new("subscriber panic"));
    });

    let _ = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        Ok(())
    })?;
    assert!(worker.join().is_err());

    let second = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Stopping);
        Ok(())
    })?;
    assert!(second.changed_state());
    assert_eq!(store.subscriber_count()?, 0);
    Ok(())
}

#[test]
fn update_events_contain_consistent_committed_snapshots() -> TestResult {
    let store = BridgeStateStore::default();
    let subscription = store.subscribe()?;
    let update = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        let _ = draft.devices_mut().insert(device("device-a", "A"))?;
        Ok(())
    })?;
    let event = subscription.recv_timeout(Duration::from_secs(1))?;

    assert_eq!(event.snapshot(), update.snapshot());
    assert_eq!(event.snapshot(), &store.snapshot()?);
    assert_eq!(event.changes().len(), 2);
    assert!(matches!(
        event.changes()[0],
        BridgeStateChange::Lifecycle { .. }
    ));
    assert!(matches!(
        event.changes()[1],
        BridgeStateChange::Devices(_)
    ));
    Ok(())
}

#[test]
fn configuration_snapshot_updates_are_typed_and_immutable() -> TestResult {
    let store = BridgeStateStore::default();
    let before = store.snapshot()?;
    let configuration = debug_configuration()?;
    let update = store.update(|draft| {
        assert!(draft.set_configuration(configuration));
        Ok(())
    })?;

    assert_eq!(before.configuration(), &BridgeConfig::default());
    assert_eq!(
        update.snapshot().configuration().logging().level(),
        LogLevel::Debug
    );
    assert!(matches!(
        update.event(),
        Some(event) if event.changes() == [BridgeStateChange::Configuration]
    ));
    Ok(())
}

#[test]
fn partial_updates_preserve_unmodified_subsystems() -> TestResult {
    let store = BridgeStateStore::default();
    let _ = store.update(|draft| {
        let _ = draft.sessions_mut().insert(session("session-a")?)?;
        let _ = draft
            .connectors_mut()
            .insert(connector("connector-a"))?;
        Ok(())
    })?;
    let before = store.snapshot()?;

    let _ = store.update(|draft| {
        let _ = draft.devices_mut().insert(device("device-a", "A"))?;
        Ok(())
    })?;
    let after = store.snapshot()?;

    assert_eq!(before.sessions(), after.sessions());
    assert_eq!(before.connectors(), after.connectors());
    assert_eq!(after.devices().len(), 1);
    Ok(())
}

#[test]
fn sequential_updates_produce_monotonic_revisions() -> TestResult {
    let store = BridgeStateStore::default();
    for expected_revision in 1..=5 {
        let update = store.update(|draft| {
            let lifecycle = match expected_revision {
                1 | 3 | 5 => BridgeLifecycleState::Running,
                _ => BridgeLifecycleState::Stopping,
            };
            let _ = draft.set_lifecycle(lifecycle);
            Ok(())
        })?;
        assert_eq!(update.snapshot().revision().get(), expected_revision);
    }
    Ok(())
}

#[test]
fn rejected_updates_propagate_errors_without_committing() -> TestResult {
    let store = BridgeStateStore::default();
    let result = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        Err(StateError::rejected("policy", "operation denied"))
    });

    assert!(matches!(
        result,
        Err(StateError::Rejected { ref code, ref message })
            if code.as_ref() == "policy" && message.as_ref() == "operation denied"
    ));
    assert_eq!(store.snapshot()?.revision(), StateRevision::INITIAL);
    assert_eq!(
        store.snapshot()?.lifecycle(),
        BridgeLifecycleState::Initializing
    );
    Ok(())
}

#[test]
fn registry_errors_propagate_through_updates_without_committing() -> TestResult {
    let store = BridgeStateStore::default();
    let result = store.update(|draft| {
        let _ = draft.devices_mut().insert(device("", "Invalid"))?;
        Ok(())
    });

    assert!(matches!(
        result,
        Err(StateError::Registry(ref error))
            if error.registry() == RegistryKind::Devices
                && error.failure() == RegistryFailure::InvalidIdentifier
    ));
    assert_eq!(store.snapshot()?.revision(), StateRevision::INITIAL);
    Ok(())
}

#[test]
fn panicking_updates_are_isolated_when_unwinding_is_enabled() -> TestResult {
    let store = BridgeStateStore::default();
    let result = store.update(|_| -> Result<(), StateError> {
        panic::resume_unwind(Box::new("update panic"));
    });

    assert_eq!(result, Err(StateError::UpdatePanicked));
    assert_eq!(store.snapshot()?.revision(), StateRevision::INITIAL);
    let recovery = store.update(|draft| {
        let _ = draft.set_lifecycle(BridgeLifecycleState::Running);
        Ok(())
    })?;
    assert_eq!(recovery.snapshot().revision().get(), 1);
    Ok(())
}

#[test]
fn store_and_snapshots_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BridgeStateStore>();
    assert_send_sync::<BridgeStateSnapshot>();
    assert_send_sync::<StateUpdate>();
}

#[test]
fn concurrent_notification_order_matches_atomic_revision_order() -> TestResult {
    let store = Arc::new(BridgeStateStore::default());
    let subscription = store.subscribe()?;
    let barrier = Arc::new(Barrier::new(5));
    let mut workers = Vec::new();

    for worker_index in 0..4 {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), StateError> {
            barrier.wait();
            for item_index in 0..25 {
                let identifier = format!("ordered-{worker_index}-{item_index}");
                store.update(|draft| {
                    let _ = draft
                        .devices_mut()
                        .insert(device(identifier, "Ordered"))?;
                    Ok(())
                })?;
            }
            Ok(())
        }));
    }

    barrier.wait();
    for expected_revision in 1..=100 {
        let event = subscription.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(event.revision().get(), expected_revision);
    }
    for worker in workers {
        join(worker)??;
    }
    Ok(())
}

#[test]
fn concurrent_read_write_stress_preserves_snapshot_invariants() -> TestResult {
    let store = Arc::new(BridgeStateStore::default());
    let finished = Arc::new(AtomicBool::new(false));
    let reader_store = Arc::clone(&store);
    let reader_finished = Arc::clone(&finished);
    let reader = thread::spawn(move || -> Result<(), StateError> {
        let mut last_revision = 0;
        while !reader_finished.load(Ordering::Acquire) {
            let snapshot = reader_store.snapshot()?;
            if snapshot.revision().get() < last_revision {
                return Err(StateError::rejected(
                    "revision-regression",
                    "snapshot revision moved backwards",
                ));
            }
            let revision = usize::try_from(snapshot.revision().get()).unwrap_or(usize::MAX);
            if snapshot.devices().len() > revision {
                return Err(StateError::rejected(
                    "snapshot-invariant",
                    "device count exceeded committed revision",
                ));
            }
            last_revision = snapshot.revision().get();
        }
        Ok(())
    });

    for index in 0..250 {
        let identifier = format!("stress-{index:03}");
        let _ = store.update(|draft| {
            let _ = draft
                .devices_mut()
                .insert(device(identifier, "Stress"))?;
            Ok(())
        })?;
    }
    finished.store(true, Ordering::Release);
    join(reader)??;

    let snapshot = store.snapshot()?;
    assert_eq!(snapshot.revision().get(), 250);
    assert_eq!(snapshot.devices().len(), 250);
    Ok(())
}
