use std::{
    collections::BTreeMap,
    fmt,
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc, Mutex, RwLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    time::Duration,
};

use crate::BridgeConfig;

use super::{
    BridgeStateData, BridgeStateDraft, BridgeStateEvent, BridgeStateSnapshot, StateError,
    StateLock, StateReceiveError, StateRevision,
};

/// Stable identifier assigned to a state subscription.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Returns the numeric subscription identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Notification delivery statistics for one committed update.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NotificationSummary {
    attempted: usize,
    delivered: usize,
    disconnected: usize,
}

impl NotificationSummary {
    /// Returns the number of subscribers considered for delivery.
    #[must_use]
    pub const fn attempted(self) -> usize {
        self.attempted
    }

    /// Returns the number of subscribers that accepted the event.
    #[must_use]
    pub const fn delivered(self) -> usize {
        self.delivered
    }

    /// Returns the number of disconnected subscribers removed during delivery.
    #[must_use]
    pub const fn disconnected(self) -> usize {
        self.disconnected
    }
}

/// Result of one transactional state update.
#[derive(Clone, Debug, PartialEq)]
pub struct StateUpdate {
    snapshot: BridgeStateSnapshot,
    event: Option<Arc<BridgeStateEvent>>,
    notifications: NotificationSummary,
}

impl StateUpdate {
    fn unchanged(snapshot: BridgeStateSnapshot) -> Self {
        Self {
            snapshot,
            event: None,
            notifications: NotificationSummary::default(),
        }
    }

    fn changed(
        snapshot: BridgeStateSnapshot,
        event: Arc<BridgeStateEvent>,
        notifications: NotificationSummary,
    ) -> Self {
        Self {
            snapshot,
            event: Some(event),
            notifications,
        }
    }

    /// Returns whether the transaction committed a new revision.
    #[must_use]
    pub const fn changed_state(&self) -> bool {
        self.event.is_some()
    }

    /// Returns the resulting immutable snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &BridgeStateSnapshot {
        &self.snapshot
    }

    /// Returns the emitted event when state changed.
    #[must_use]
    pub fn event(&self) -> Option<&BridgeStateEvent> {
        self.event.as_deref()
    }

    /// Returns notification delivery statistics.
    #[must_use]
    pub const fn notifications(&self) -> NotificationSummary {
        self.notifications
    }
}

struct StoreInner {
    state: RwLock<Arc<BridgeStateData>>,
    revision: AtomicU64,
    subscribers: Mutex<BTreeMap<SubscriptionId, Sender<Arc<BridgeStateEvent>>>>,
    next_subscription_id: AtomicU64,
}

impl fmt::Debug for StoreInner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoreInner")
            .field("revision", &self.revision.load(Ordering::Relaxed))
            .field(
                "next_subscription_id",
                &self.next_subscription_id.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

/// Thread-safe single source of truth for Bridge runtime state.
#[derive(Clone, Debug)]
pub struct BridgeStateStore {
    inner: Arc<StoreInner>,
}

impl BridgeStateStore {
    /// Creates a state store with revision zero and empty registries.
    #[must_use]
    pub fn new(configuration: BridgeConfig) -> Self {
        Self {
            inner: Arc::new(StoreInner {
                state: RwLock::new(Arc::new(BridgeStateData::new(configuration))),
                revision: AtomicU64::new(StateRevision::INITIAL.get()),
                subscribers: Mutex::new(BTreeMap::new()),
                next_subscription_id: AtomicU64::new(1),
            }),
        }
    }

    /// Returns a deterministic immutable snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StateError::LockPoisoned`] when the state lock was poisoned.
    pub fn snapshot(&self) -> Result<BridgeStateSnapshot, StateError> {
        let state = self
            .inner
            .state
            .read()
            .map_err(|_| StateError::LockPoisoned {
                lock: StateLock::State,
            })?;
        let revision = StateRevision::new(self.inner.revision.load(Ordering::Acquire));
        Ok(BridgeStateSnapshot::new(revision, Arc::clone(&*state)))
    }

    /// Applies one atomic transaction and publishes a typed event only when state changes.
    ///
    /// The update closure runs while the state write lock is held. Panics are isolated when the
    /// build uses unwinding and are returned as [`StateError::UpdatePanicked`].
    ///
    /// # Errors
    ///
    /// Returns a structured state error when the closure rejects the transaction, a registry
    /// operation fails, synchronization is poisoned, or a monotonic counter is exhausted.
    pub fn update(
        &self,
        operation: impl FnOnce(&mut BridgeStateDraft) -> Result<(), StateError>,
    ) -> Result<StateUpdate, StateError> {
        self.update_with(operation)
    }

    /// Applies one atomic transaction with a caller-defined structured error type.
    ///
    /// The error type must accept [`StateError`] so synchronization, panic-isolation, and revision
    /// failures remain structured while domain validation errors can propagate without lossy
    /// conversion.
    ///
    /// # Errors
    ///
    /// Returns either the caller's domain error or a converted [`StateError`].
    pub fn update_with<E>(
        &self,
        operation: impl FnOnce(&mut BridgeStateDraft) -> Result<(), E>,
    ) -> Result<StateUpdate, E>
    where
        E: From<StateError>,
    {
        let mut state = self.inner.state.write().map_err(|_| {
            E::from(StateError::LockPoisoned {
                lock: StateLock::State,
            })
        })?;
        let before = Arc::clone(&*state);
        let mut draft = BridgeStateDraft::from_data(before.as_ref());

        match panic::catch_unwind(AssertUnwindSafe(|| operation(&mut draft))) {
            Ok(result) => result?,
            Err(_) => return Err(E::from(StateError::UpdatePanicked)),
        }

        let (after_data, session_transitions, discovery_events) = draft.into_parts();
        let after = Arc::new(after_data);
        let previous_revision = StateRevision::new(self.inner.revision.load(Ordering::Relaxed));

        if before.as_ref() == after.as_ref() {
            return Ok(StateUpdate::unchanged(BridgeStateSnapshot::new(
                previous_revision,
                before,
            )));
        }

        let next_revision = previous_revision
            .get()
            .checked_add(1)
            .map(StateRevision::new)
            .ok_or_else(|| E::from(StateError::RevisionExhausted))?;
        let mut subscribers = self.inner.subscribers.lock().map_err(|_| {
            E::from(StateError::LockPoisoned {
                lock: StateLock::Subscribers,
            })
        })?;

        *state = Arc::clone(&after);
        self.inner
            .revision
            .store(next_revision.get(), Ordering::Release);

        let snapshot = BridgeStateSnapshot::new(next_revision, Arc::clone(&after));
        let event = Arc::new(BridgeStateEvent::between(
            previous_revision,
            next_revision,
            before.as_ref(),
            after.as_ref(),
            session_transitions,
            discovery_events,
            snapshot.clone(),
        ));
        let notifications = notify_subscribers(&mut subscribers, &event);

        Ok(StateUpdate::changed(snapshot, event, notifications))
    }

    /// Creates a race-free subscription and captures its initial snapshot.
    ///
    /// # Errors
    ///
    /// Returns a structured state error when synchronization is poisoned or subscription
    /// identifiers are exhausted.
    pub fn subscribe(&self) -> Result<BridgeStateSubscription, StateError> {
        let state = self
            .inner
            .state
            .read()
            .map_err(|_| StateError::LockPoisoned {
                lock: StateLock::State,
            })?;
        let mut subscribers =
            self.inner
                .subscribers
                .lock()
                .map_err(|_| StateError::LockPoisoned {
                    lock: StateLock::Subscribers,
                })?;
        let identifier = allocate_identifier(&self.inner.next_subscription_id)?;
        let revision = StateRevision::new(self.inner.revision.load(Ordering::Acquire));
        let initial_snapshot = BridgeStateSnapshot::new(revision, Arc::clone(&*state));
        let (sender, receiver) = mpsc::channel();
        subscribers.insert(identifier, sender);

        Ok(BridgeStateSubscription {
            identifier,
            initial_snapshot,
            receiver,
            store: Arc::downgrade(&self.inner),
            active: AtomicBool::new(true),
        })
    }

    /// Returns the number of active subscribers.
    ///
    /// # Errors
    ///
    /// Returns [`StateError::LockPoisoned`] when the subscriber registry is poisoned.
    pub fn subscriber_count(&self) -> Result<usize, StateError> {
        self.inner
            .subscribers
            .lock()
            .map(|subscribers| subscribers.len())
            .map_err(|_| StateError::LockPoisoned {
                lock: StateLock::Subscribers,
            })
    }
}

impl Default for BridgeStateStore {
    fn default() -> Self {
        Self::new(BridgeConfig::default())
    }
}

fn allocate_identifier(counter: &AtomicU64) -> Result<SubscriptionId, StateError> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(SubscriptionId)
        .map_err(|_| StateError::SubscriptionIdExhausted)
}

fn notify_subscribers(
    subscribers: &mut BTreeMap<SubscriptionId, Sender<Arc<BridgeStateEvent>>>,
    event: &Arc<BridgeStateEvent>,
) -> NotificationSummary {
    let attempted = subscribers.len();
    let disconnected = subscribers
        .iter()
        .filter_map(|(identifier, sender)| {
            sender
                .send(Arc::clone(event))
                .is_err()
                .then_some(*identifier)
        })
        .collect::<Vec<_>>();
    let disconnected_count = disconnected.len();

    for identifier in disconnected {
        subscribers.remove(&identifier);
    }

    NotificationSummary {
        attempted,
        delivered: attempted.saturating_sub(disconnected_count),
        disconnected: disconnected_count,
    }
}

/// Message-passing state subscription with race-free initial snapshot capture.
pub struct BridgeStateSubscription {
    identifier: SubscriptionId,
    initial_snapshot: BridgeStateSnapshot,
    receiver: Receiver<Arc<BridgeStateEvent>>,
    store: Weak<StoreInner>,
    active: AtomicBool,
}

impl BridgeStateSubscription {
    /// Returns the subscription identifier.
    #[must_use]
    pub const fn identifier(&self) -> SubscriptionId {
        self.identifier
    }

    /// Returns the snapshot captured atomically with subscription registration.
    #[must_use]
    pub const fn initial_snapshot(&self) -> &BridgeStateSnapshot {
        &self.initial_snapshot
    }

    /// Blocks until the next event arrives.
    ///
    /// # Errors
    ///
    /// Returns [`StateReceiveError::Disconnected`] after the store or subscription disconnects.
    pub fn recv(&self) -> Result<Arc<BridgeStateEvent>, StateReceiveError> {
        self.receiver
            .recv()
            .map_err(|_| StateReceiveError::Disconnected)
    }

    /// Attempts to receive the next event without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`StateReceiveError::Empty`] when no event is queued and
    /// [`StateReceiveError::Disconnected`] when the channel has closed.
    pub fn try_recv(&self) -> Result<Arc<BridgeStateEvent>, StateReceiveError> {
        self.receiver.try_recv().map_err(|source| match source {
            TryRecvError::Empty => StateReceiveError::Empty,
            TryRecvError::Disconnected => StateReceiveError::Disconnected,
        })
    }

    /// Waits up to `timeout` for the next event.
    ///
    /// # Errors
    ///
    /// Returns [`StateReceiveError::Timeout`] when the deadline expires and
    /// [`StateReceiveError::Disconnected`] when the channel has closed.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Arc<BridgeStateEvent>, StateReceiveError> {
        self.receiver
            .recv_timeout(timeout)
            .map_err(|source| match source {
                RecvTimeoutError::Timeout => StateReceiveError::Timeout,
                RecvTimeoutError::Disconnected => StateReceiveError::Disconnected,
            })
    }

    /// Unregisters this subscription.
    ///
    /// Returns `true` only when an active subscriber was removed.
    ///
    /// # Errors
    ///
    /// Returns [`StateError::LockPoisoned`] when the subscriber registry is poisoned.
    pub fn unsubscribe(&self) -> Result<bool, StateError> {
        if self
            .active
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(false);
        }

        let Some(store) = self.store.upgrade() else {
            return Ok(false);
        };
        let Ok(mut subscribers) = store.subscribers.lock() else {
            self.active.store(true, Ordering::Release);
            return Err(StateError::LockPoisoned {
                lock: StateLock::Subscribers,
            });
        };
        let removed = subscribers.remove(&self.identifier).is_some();
        drop(subscribers);

        while self.receiver.try_recv().is_ok() {}
        Ok(removed)
    }
}

impl fmt::Debug for BridgeStateSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BridgeStateSubscription")
            .field("identifier", &self.identifier)
            .field("initial_snapshot", &self.initial_snapshot)
            .field("active", &self.active.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Drop for BridgeStateSubscription {
    fn drop(&mut self) {
        if !self.active.swap(false, Ordering::AcqRel) {
            return;
        }
        let Some(store) = self.store.upgrade() else {
            return;
        };
        match store.subscribers.lock() {
            Ok(mut subscribers) => {
                subscribers.remove(&self.identifier);
            }
            Err(poisoned) => {
                poisoned.into_inner().remove(&self.identifier);
            }
        }
    }
}
