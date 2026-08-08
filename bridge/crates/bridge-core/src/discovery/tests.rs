use std::{
    collections::BTreeMap,
    error::Error,
    future::Future,
    io,
    pin::Pin,
    sync::{Arc, Barrier},
    thread,
};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::*;

const SOURCE: &str = "manual-test";
const TRANSPORT: &str = "secure-lan";

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn version(major: u32, minor: u32, patch: u32) -> ProtocolVersion {
    ProtocolVersion {
        major,
        minor,
        patch,
    }
}

fn capabilities(items: &[Capability]) -> TestResult<DiscoveryCapabilities> {
    Ok(DiscoveryCapabilities::new(CapabilitySet {
        supported: items.iter().map(|item| *item as i32).collect(),
        required: Vec::new(),
        parameters: BTreeMap::new(),
    })?)
}

fn policy() -> TestResult<DiscoveryPolicy> {
    Ok(DiscoveryPolicy::new(
        vec![version(1, 0, 0), version(1, 1, 0), version(2, 0, 0)],
        vec![Capability::CapabilityPlaybackRead as i32],
        10_000,
        500,
        8,
        1_024,
    )?)
}

fn source(value: &str) -> TestResult<DiscoverySource> {
    Ok(DiscoverySource::new(value)?)
}

fn transport(value: &str) -> TestResult<TransportId> {
    Ok(TransportId::new(value)?)
}

fn bridge(value: &str) -> TestResult<BridgeId> {
    Ok(BridgeId::new(value)?)
}

fn advertisement(
    bridge_id: &str,
    transport_id: &str,
    discovered_at: u64,
    expires_at: u64,
    bridge_version: &str,
) -> TestResult<DiscoveryAdvertisement> {
    let mut metadata = BTreeMap::new();
    metadata.insert("instance".to_owned(), bridge_id.as_bytes().to_vec());
    Ok(DiscoveryAdvertisement::new(
        bridge(bridge_id)?,
        transport(transport_id)?,
        vec![version(2, 0, 0), version(1, 1, 0), version(1, 0, 0)],
        capabilities(&[
            Capability::CapabilityPlaybackRead,
            Capability::CapabilityPlay,
            Capability::CapabilityPause,
        ])?,
        bridge_version,
        DiscoveryTimestamp::from_unix_millis(discovered_at),
        DiscoveryTimestamp::from_unix_millis(expires_at),
        AdvertisementSignatureMetadata::signed("ed25519", "bridge-key", [7_u8; 64])?,
        metadata,
    )?)
}

fn new_manager() -> TestResult<(BridgeStateStore, DiscoveryManager)> {
    let state = BridgeStateStore::default();
    let manager = DiscoveryManager::new(state.clone(), policy()?);
    Ok((state, manager))
}

fn receive(
    manager: &DiscoveryManager,
    bridge_id: &str,
    discovered_at: u64,
    expires_at: u64,
    observed_at: u64,
    expected_revision: Option<DiscoveryRevision>,
) -> TestResult<DiscoveryMutation> {
    Ok(
        manager.receive_advertisement(ReceiveDiscoveryAdvertisement::new(
            source(SOURCE)?,
            advertisement(bridge_id, TRANSPORT, discovered_at, expires_at, "0.1.0")?,
            DiscoveryTimestamp::from_unix_millis(observed_at),
            expected_revision,
        ))?,
    )
}

fn peer_key(bridge_id: &str) -> TestResult<DiscoveryPeerKey> {
    Ok(DiscoveryPeerKey::new(
        bridge(bridge_id)?,
        source(SOURCE)?,
        transport(TRANSPORT)?,
    ))
}

fn validate(
    manager: &DiscoveryManager,
    key: &DiscoveryPeerKey,
    revision: u64,
    timestamp: u64,
) -> TestResult<DiscoveryMutation> {
    Ok(manager.validate_peer(ValidateDiscoveredPeer::new(
        key.clone(),
        DiscoveryRevision::new(revision),
        DiscoveryTimestamp::from_unix_millis(timestamp),
    ))?)
}

fn transition(
    manager: &DiscoveryManager,
    key: &DiscoveryPeerKey,
    revision: u64,
    state: DiscoveryState,
    timestamp: u64,
) -> TestResult<DiscoveryMutation> {
    Ok(manager.transition_peer(TransitionDiscoveredPeer::new(
        key.clone(),
        DiscoveryRevision::new(revision),
        state,
        DiscoveryTimestamp::from_unix_millis(timestamp),
    ))?)
}

fn join<T>(handle: thread::JoinHandle<T>) -> TestResult<T> {
    handle
        .join()
        .map_err(|_| io::Error::other("discovery test worker panicked").into())
}

#[test]
fn advertisements_are_validated_immutable_and_deterministic() -> TestResult {
    let first = advertisement("bridge-a", TRANSPORT, 100, 1_000, "1.2.3")?;
    let second = advertisement("bridge-a", TRANSPORT, 100, 1_000, "1.2.3")?;

    assert_eq!(first, second);
    assert_eq!(first.bridge_version(), "1.2.3");
    assert_eq!(first.lifetime_ms(), 900);
    assert_eq!(
        first
            .supported_protocol_versions()
            .iter()
            .map(|item| (item.major, item.minor, item.patch))
            .collect::<Vec<_>>(),
        vec![(1, 0, 0), (1, 1, 0), (2, 0, 0)]
    );
    assert_eq!(
        first.capabilities().supported().collect::<Vec<_>>(),
        vec![
            Capability::CapabilityPlaybackRead as i32,
            Capability::CapabilityPlay as i32,
            Capability::CapabilityPause as i32,
        ]
    );
    assert_eq!(
        first
            .provider_metadata()
            .keys()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>(),
        vec!["instance"]
    );
    Ok(())
}

#[test]
fn advertisement_validation_rejects_invalid_values() -> TestResult {
    let result = DiscoveryAdvertisement::new(
        bridge("bridge-a")?,
        transport(TRANSPORT)?,
        vec![],
        capabilities(&[Capability::CapabilityPlaybackRead])?,
        "0.1.0",
        DiscoveryTimestamp::from_unix_millis(100),
        DiscoveryTimestamp::from_unix_millis(200),
        AdvertisementSignatureMetadata::Unsigned,
        BTreeMap::new(),
    );
    assert_eq!(result, Err(DiscoveryModelError::EmptyProtocolVersions));

    let result = DiscoveryAdvertisement::new(
        bridge("bridge-a")?,
        transport(TRANSPORT)?,
        vec![version(1, 0, 0)],
        capabilities(&[Capability::CapabilityPlaybackRead])?,
        "0.1.0",
        DiscoveryTimestamp::from_unix_millis(200),
        DiscoveryTimestamp::from_unix_millis(200),
        AdvertisementSignatureMetadata::Unsigned,
        BTreeMap::new(),
    );
    assert_eq!(result, Err(DiscoveryModelError::InvalidAdvertisementWindow));
    Ok(())
}

#[test]
fn policy_enforces_expiration_future_skew_lifetime_capabilities_and_protocols() -> TestResult {
    let (_state, manager) = new_manager()?;
    let expired = manager.receive_advertisement(ReceiveDiscoveryAdvertisement::new(
        source(SOURCE)?,
        advertisement("expired", TRANSPORT, 100, 200, "0.1.0")?,
        DiscoveryTimestamp::from_unix_millis(200),
        None,
    ));
    assert!(matches!(
        expired,
        Err(DiscoveryError::AdvertisementExpired { .. })
    ));

    let future = manager.receive_advertisement(ReceiveDiscoveryAdvertisement::new(
        source(SOURCE)?,
        advertisement("future", TRANSPORT, 2_000, 3_000, "0.1.0")?,
        DiscoveryTimestamp::from_unix_millis(1_000),
        None,
    ));
    assert!(matches!(
        future,
        Err(DiscoveryError::AdvertisementFromFuture { .. })
    ));

    let incompatible = DiscoveryAdvertisement::new(
        bridge("incompatible")?,
        transport(TRANSPORT)?,
        vec![version(9, 0, 0)],
        capabilities(&[Capability::CapabilityPlaybackRead])?,
        "0.1.0",
        DiscoveryTimestamp::from_unix_millis(100),
        DiscoveryTimestamp::from_unix_millis(500),
        AdvertisementSignatureMetadata::Unsigned,
        BTreeMap::new(),
    )?;
    let result = manager.receive_advertisement(ReceiveDiscoveryAdvertisement::new(
        source(SOURCE)?,
        incompatible,
        DiscoveryTimestamp::from_unix_millis(150),
        None,
    ));
    assert!(matches!(
        result,
        Err(DiscoveryError::NoCompatibleProtocolVersion { .. })
    ));
    Ok(())
}

#[test]
fn state_machine_accepts_every_legal_transition() {
    let states = [
        DiscoveryState::Idle,
        DiscoveryState::Discovering,
        DiscoveryState::AdvertisementReceived,
        DiscoveryState::Validated,
        DiscoveryState::Available,
        DiscoveryState::Unavailable,
        DiscoveryState::Expired,
        DiscoveryState::Removed,
    ];
    let legal = [
        (DiscoveryState::Idle, DiscoveryState::Discovering),
        (DiscoveryState::Idle, DiscoveryState::AdvertisementReceived),
        (
            DiscoveryState::Discovering,
            DiscoveryState::AdvertisementReceived,
        ),
        (DiscoveryState::Discovering, DiscoveryState::Unavailable),
        (DiscoveryState::Discovering, DiscoveryState::Expired),
        (
            DiscoveryState::AdvertisementReceived,
            DiscoveryState::Validated,
        ),
        (
            DiscoveryState::AdvertisementReceived,
            DiscoveryState::Unavailable,
        ),
        (
            DiscoveryState::AdvertisementReceived,
            DiscoveryState::Expired,
        ),
        (
            DiscoveryState::Validated,
            DiscoveryState::AdvertisementReceived,
        ),
        (DiscoveryState::Validated, DiscoveryState::Available),
        (DiscoveryState::Validated, DiscoveryState::Unavailable),
        (DiscoveryState::Validated, DiscoveryState::Expired),
        (
            DiscoveryState::Available,
            DiscoveryState::AdvertisementReceived,
        ),
        (DiscoveryState::Available, DiscoveryState::Unavailable),
        (DiscoveryState::Available, DiscoveryState::Expired),
        (DiscoveryState::Unavailable, DiscoveryState::Idle),
        (DiscoveryState::Unavailable, DiscoveryState::Discovering),
        (
            DiscoveryState::Unavailable,
            DiscoveryState::AdvertisementReceived,
        ),
        (DiscoveryState::Unavailable, DiscoveryState::Expired),
    ];

    for previous in states {
        for requested in states {
            assert_eq!(
                previous.can_transition_to(requested),
                legal.contains(&(previous, requested)),
                "unexpected transition result for {previous:?} -> {requested:?}"
            );
        }
    }
}

#[test]
fn terminal_states_remain_terminal() {
    for state in [DiscoveryState::Expired, DiscoveryState::Removed] {
        assert!(state.is_terminal());
        for requested in [
            DiscoveryState::Idle,
            DiscoveryState::Discovering,
            DiscoveryState::AdvertisementReceived,
            DiscoveryState::Validated,
            DiscoveryState::Available,
            DiscoveryState::Unavailable,
            DiscoveryState::Expired,
            DiscoveryState::Removed,
        ] {
            assert!(!state.can_transition_to(requested));
        }
    }
}

#[test]
fn registry_supports_insertion_replacement_lookup_and_removal() -> TestResult {
    let (_state, manager) = new_manager()?;
    let created = receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    let key = peer_key("bridge-a")?;
    assert_eq!(
        created.peer().map(DiscoveredPeer::revision),
        Some(DiscoveryRevision::INITIAL)
    );
    assert!(manager.peer_exists(&key)?);
    assert_eq!(
        manager.lookup_peer(&key)?.map(|peer| peer.state()),
        Some(DiscoveryState::AdvertisementReceived)
    );

    let refreshed = receive(
        &manager,
        "bridge-a",
        200,
        1_100,
        250,
        Some(DiscoveryRevision::INITIAL),
    )?;
    assert_eq!(
        refreshed.peer().map(DiscoveredPeer::revision),
        Some(DiscoveryRevision::new(1))
    );
    assert_eq!(manager.list_peers()?.len(), 1);

    let removed = manager.remove_peer(RemoveDiscoveredPeer::new(
        key.clone(),
        DiscoveryRevision::new(1),
        DiscoveryTimestamp::from_unix_millis(300),
    ))?;
    assert!(removed.peer().is_none());
    assert!(!manager.peer_exists(&key)?);
    Ok(())
}

#[test]
fn manager_enforces_full_lifecycle_and_expiration_boundary() -> TestResult {
    let (_state, manager) = new_manager()?;
    let key = peer_key("bridge-a")?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    validate(&manager, &key, 0, 200)?;
    transition(&manager, &key, 1, DiscoveryState::Available, 250)?;
    transition(&manager, &key, 2, DiscoveryState::Unavailable, 300)?;
    transition(&manager, &key, 3, DiscoveryState::Discovering, 350)?;

    let premature = transition(&manager, &key, 4, DiscoveryState::Expired, 999);
    assert!(
        matches!(premature, Err(ref error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::ExpirationNotReached { .. })))
    );

    transition(&manager, &key, 4, DiscoveryState::Expired, 1_000)?;
    let terminal = transition(&manager, &key, 5, DiscoveryState::Unavailable, 1_001);
    assert!(
        matches!(terminal, Err(ref error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::TerminalPeer { .. })))
    );
    Ok(())
}

#[test]
fn direct_transitions_cannot_bypass_receipt_validation_or_removal() -> TestResult {
    let (_state, manager) = new_manager()?;
    let key = peer_key("bridge-a")?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;

    for state in [
        DiscoveryState::AdvertisementReceived,
        DiscoveryState::Validated,
        DiscoveryState::Removed,
    ] {
        let result = transition(&manager, &key, 0, state, 200);
        assert!(
            matches!(result, Err(ref error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::InvalidTransition { .. })))
        );
    }
    Ok(())
}

#[test]
fn filtering_is_typed_customizable_and_deterministic() -> TestResult {
    let (_state, manager) = new_manager()?;
    receive(&manager, "bridge-c", 100, 1_000, 150, None)?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    receive(&manager, "bridge-b", 100, 1_000, 150, None)?;

    let filter = DiscoveryFilter::new()
        .with_transport_id(transport(TRANSPORT)?)
        .with_source(source(SOURCE)?)
        .with_protocol_version(&version(2, 0, 0))
        .requiring_capability(Capability::CapabilityPlay as i32);
    let peers =
        manager.filter_peers_with(&filter, |peer| peer.bridge_id().as_str() != "bridge-b")?;
    assert_eq!(
        peers
            .iter()
            .map(|peer| peer.bridge_id().as_str())
            .collect::<Vec<_>>(),
        vec!["bridge-a", "bridge-c"]
    );
    Ok(())
}

#[test]
fn snapshots_are_immutable_and_registry_order_is_stable() -> TestResult {
    let (_state, manager) = new_manager()?;
    receive(&manager, "bridge-z", 100, 1_000, 150, None)?;
    let before = manager.snapshot()?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    let after = manager.snapshot()?;

    assert_eq!(before.peers().len(), 1);
    assert_eq!(after.peers().len(), 2);
    assert_eq!(
        after
            .peers()
            .iter()
            .map(|peer| peer.bridge_id().as_str())
            .collect::<Vec<_>>(),
        vec!["bridge-a", "bridge-z"]
    );
    assert_eq!(before.state_revision().get(), 1);
    assert_eq!(after.state_revision().get(), 2);
    Ok(())
}

#[test]
fn bridge_state_events_include_discovery_change_and_insert_delta() -> TestResult {
    let (_state, manager) = new_manager()?;
    let mutation = receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    let event = mutation
        .state_update()
        .event()
        .ok_or_else(|| io::Error::other("missing discovery event"))?;

    assert_eq!(event.previous_revision(), StateRevision::INITIAL);
    assert_eq!(event.revision().get(), 1);
    assert_eq!(event.snapshot(), mutation.state_update().snapshot());
    assert_eq!(event.changes().len(), 2);
    assert!(matches!(
        &event.changes()[0],
        BridgeStateChange::Discovery(discovery)
            if discovery.peer_revision() == DiscoveryRevision::INITIAL
                && matches!(
                    discovery.kind(),
                    DiscoveryEventKind::AdvertisementReceived {
                        previous: None,
                        current: DiscoveryState::AdvertisementReceived,
                        advertisement_changed: true,
                    }
                )
    ));
    assert!(matches!(
        &event.changes()[1],
        BridgeStateChange::Discoveries(delta)
            if delta.inserted() == [peer_key("bridge-a")?]
                && delta.replaced().is_empty()
                && delta.removed().is_empty()
    ));
    Ok(())
}

#[test]
fn bridge_state_events_include_replacement_and_removal_deltas() -> TestResult {
    let (_state, manager) = new_manager()?;
    let key = peer_key("bridge-a")?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    let refreshed = receive(
        &manager,
        "bridge-a",
        200,
        1_100,
        250,
        Some(DiscoveryRevision::INITIAL),
    )?;
    assert!(matches!(
        refreshed.state_update().event().map(BridgeStateEvent::changes),
        Some(changes)
            if matches!(&changes[1], BridgeStateChange::Discoveries(delta) if delta.replaced() == [key.clone()])
    ));

    let removed = manager.remove_peer(RemoveDiscoveredPeer::new(
        key.clone(),
        DiscoveryRevision::new(1),
        DiscoveryTimestamp::from_unix_millis(300),
    ))?;
    let changes = removed
        .state_update()
        .event()
        .ok_or_else(|| io::Error::other("missing removal event"))?
        .changes();
    assert!(matches!(
        &changes[0],
        BridgeStateChange::Discovery(discovery)
            if discovery.peer_revision() == DiscoveryRevision::new(2)
                && discovery.timestamp() == DiscoveryTimestamp::from_unix_millis(300)
                && matches!(
                    discovery.kind(),
                    DiscoveryEventKind::Removed {
                        previous: DiscoveryState::AdvertisementReceived,
                        current: DiscoveryState::Removed,
                    }
                )
    ));
    assert!(matches!(
        &changes[1],
        BridgeStateChange::Discoveries(delta) if delta.removed() == [key]
    ));
    Ok(())
}

#[test]
fn expiration_sweep_is_deterministic_and_updates_one_state_revision() -> TestResult {
    let (_state, manager) = new_manager()?;
    receive(&manager, "bridge-c", 100, 500, 150, None)?;
    receive(&manager, "bridge-a", 100, 500, 150, None)?;
    receive(&manager, "bridge-b", 100, 900, 150, None)?;
    let before_revision = manager.snapshot()?.state_revision();

    let expired = manager.expire_peers(ExpireDiscoveredPeers::new(
        DiscoveryTimestamp::from_unix_millis(500),
    ))?;
    assert_eq!(
        expired
            .peer_keys()
            .iter()
            .map(|key| key.bridge_id().as_str())
            .collect::<Vec<_>>(),
        vec!["bridge-a", "bridge-c"]
    );
    assert_eq!(
        expired.state_update().snapshot().revision().get(),
        before_revision.get() + 1
    );
    let changes = expired
        .state_update()
        .event()
        .ok_or_else(|| io::Error::other("missing expiration event"))?
        .changes();
    assert!(
        matches!(&changes[0], BridgeStateChange::Discovery(event) if event.peer_key().bridge_id().as_str() == "bridge-a")
    );
    assert!(
        matches!(&changes[1], BridgeStateChange::Discovery(event) if event.peer_key().bridge_id().as_str() == "bridge-c")
    );
    assert!(
        matches!(&changes[2], BridgeStateChange::Discoveries(delta) if delta.replaced().len() == 2)
    );
    Ok(())
}

#[test]
fn stale_refresh_rolls_back_without_revision_or_event() -> TestResult {
    let (state, manager) = new_manager()?;
    let key = peer_key("bridge-a")?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    receive(
        &manager,
        "bridge-a",
        200,
        1_100,
        250,
        Some(DiscoveryRevision::INITIAL),
    )?;
    let before = state.snapshot()?;
    let stale_result = receive(
        &manager,
        "bridge-a",
        300,
        1_200,
        350,
        Some(DiscoveryRevision::INITIAL),
    );
    assert!(
        matches!(stale_result, Err(ref error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::StaleRevision { .. })))
    );
    let after = state.snapshot()?;
    assert_eq!(before, after);
    assert_eq!(
        after.discoveries().get(&key).map(DiscoveredPeer::revision),
        Some(DiscoveryRevision::new(1))
    );
    Ok(())
}

#[test]
fn conflicting_and_regressing_advertisements_are_rejected_atomically() -> TestResult {
    let (state, manager) = new_manager()?;
    receive(&manager, "bridge-a", 200, 1_000, 250, None)?;
    let before = state.snapshot()?;

    let regression = receive(
        &manager,
        "bridge-a",
        100,
        900,
        300,
        Some(DiscoveryRevision::INITIAL),
    );
    assert!(
        matches!(regression, Err(ref error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::AdvertisementTimestampRegression { .. })))
    );

    let conflicting = manager.receive_advertisement(ReceiveDiscoveryAdvertisement::new(
        source(SOURCE)?,
        advertisement("bridge-a", TRANSPORT, 200, 1_100, "0.2.0")?,
        DiscoveryTimestamp::from_unix_millis(300),
        Some(DiscoveryRevision::INITIAL),
    ));
    assert!(matches!(
        conflicting,
        Err(DiscoveryError::AdvertisementConflict { .. })
    ));
    assert_eq!(before, state.snapshot()?);
    Ok(())
}

#[test]
fn identical_inputs_produce_identical_snapshots_and_events() -> TestResult {
    let (first_state, first_manager) = new_manager()?;
    let (second_state, second_manager) = new_manager()?;

    for manager in [&first_manager, &second_manager] {
        receive(manager, "bridge-b", 100, 500, 150, None)?;
        receive(manager, "bridge-a", 100, 500, 150, None)?;
    }
    let first = first_manager.expire_peers(ExpireDiscoveredPeers::new(
        DiscoveryTimestamp::from_unix_millis(500),
    ))?;
    let second = second_manager.expire_peers(ExpireDiscoveredPeers::new(
        DiscoveryTimestamp::from_unix_millis(500),
    ))?;

    assert_eq!(first_state.snapshot()?, second_state.snapshot()?);
    assert_eq!(first.state_update(), second.state_update());
    Ok(())
}

#[test]
fn concurrent_first_receipt_has_one_winner() -> TestResult {
    let (_state, manager) = new_manager()?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            receive(&manager, "bridge-a", 100, 1_000, 150, None)
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
            .filter(|result| matches!(result, Err(error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::RevisionRequired { .. }))))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn concurrent_refresh_has_one_stale_revision_loser() -> TestResult {
    let (_state, manager) = new_manager()?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for offset in [0_u64, 1] {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            receive(
                &manager,
                "bridge-a",
                200 + offset,
                1_100 + offset,
                250 + offset,
                Some(DiscoveryRevision::INITIAL),
            )
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
            .filter(|result| matches!(result, Err(error) if error.downcast_ref::<DiscoveryError>().is_some_and(|value| matches!(value, DiscoveryError::StaleRevision { .. }))))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn concurrent_expiration_is_idempotent() -> TestResult {
    let (_state, manager) = new_manager()?;
    receive(&manager, "bridge-a", 100, 500, 150, None)?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            manager.expire_peers(ExpireDiscoveredPeers::new(
                DiscoveryTimestamp::from_unix_millis(500),
            ))
        }));
    }
    barrier.wait();
    let results = workers
        .into_iter()
        .map(join)
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 2);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().ok())
            .filter(|result| result.state_update().changed_state())
            .count(),
        1
    );
    Ok(())
}

#[test]
fn concurrent_removal_has_one_winner() -> TestResult {
    let (_state, manager) = new_manager()?;
    let key = peer_key("bridge-a")?;
    receive(&manager, "bridge-a", 100, 1_000, 150, None)?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();

    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        let key = key.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            manager.remove_peer(RemoveDiscoveredPeer::new(
                key,
                DiscoveryRevision::INITIAL,
                DiscoveryTimestamp::from_unix_millis(200),
            ))
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
            .filter(|result| matches!(result, Err(DiscoveryError::PeerNotFound { .. })))
            .count(),
        1
    );
    Ok(())
}

#[derive(Debug)]
struct TestProvider {
    source: DiscoverySource,
}

impl DiscoveryProvider for TestProvider {
    fn source(&self) -> &DiscoverySource {
        &self.source
    }

    fn start<'a>(
        &'a self,
        _filter: &'a DiscoveryFilter,
    ) -> DiscoveryFuture<'a, DiscoveryResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn next_advertisement(
        &self,
    ) -> DiscoveryFuture<'_, DiscoveryResult<Option<DiscoveryAdvertisement>>> {
        Box::pin(async { Ok(None) })
    }

    fn validate_advertisement<'a>(
        &'a self,
        _advertisement: &'a DiscoveryAdvertisement,
    ) -> DiscoveryFuture<'a, DiscoveryResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn stop(&self) -> DiscoveryFuture<'_, DiscoveryResult<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn provider_contract_is_object_safe_runtime_neutral_send_and_sync() -> TestResult {
    fn assert_send_sync<T: Send + Sync>() {}
    fn accept_provider(_provider: &dyn DiscoveryProvider) {}
    fn accept_future(
        future: DiscoveryFuture<'_, DiscoveryResult<()>>,
    ) -> Pin<Box<dyn Future<Output = DiscoveryResult<()>> + Send + '_>> {
        future
    }

    assert_send_sync::<DiscoveryManager>();
    assert_send_sync::<DiscoverySnapshot>();
    let provider = TestProvider {
        source: source(SOURCE)?,
    };
    accept_provider(&provider);
    let _future = accept_future(provider.stop());
    Ok(())
}
