use crate::{DiscoveryPeerKey, DiscoveryRevision, DiscoveryState, DiscoveryTimestamp};

/// Typed discovery change attached to one committed Bridge State event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryEvent {
    peer_key: DiscoveryPeerKey,
    kind: DiscoveryEventKind,
    peer_revision: DiscoveryRevision,
    timestamp: DiscoveryTimestamp,
}

impl DiscoveryEvent {
    pub(crate) fn advertisement(
        peer_key: DiscoveryPeerKey,
        previous: Option<DiscoveryState>,
        advertisement_changed: bool,
        peer_revision: DiscoveryRevision,
        timestamp: DiscoveryTimestamp,
    ) -> Self {
        Self {
            peer_key,
            kind: DiscoveryEventKind::AdvertisementReceived {
                previous,
                current: DiscoveryState::AdvertisementReceived,
                advertisement_changed,
            },
            peer_revision,
            timestamp,
        }
    }

    pub(crate) fn lifecycle(
        peer_key: DiscoveryPeerKey,
        previous: DiscoveryState,
        current: DiscoveryState,
        peer_revision: DiscoveryRevision,
        timestamp: DiscoveryTimestamp,
    ) -> Self {
        Self {
            peer_key,
            kind: DiscoveryEventKind::Lifecycle { previous, current },
            peer_revision,
            timestamp,
        }
    }

    pub(crate) fn removed(
        peer_key: DiscoveryPeerKey,
        previous: DiscoveryState,
        peer_revision: DiscoveryRevision,
        timestamp: DiscoveryTimestamp,
    ) -> Self {
        Self {
            peer_key,
            kind: DiscoveryEventKind::Removed {
                previous,
                current: DiscoveryState::Removed,
            },
            peer_revision,
            timestamp,
        }
    }

    /// Returns the affected provider-specific peer key.
    #[must_use]
    pub const fn peer_key(&self) -> &DiscoveryPeerKey {
        &self.peer_key
    }

    /// Returns the typed discovery event kind.
    #[must_use]
    pub const fn kind(&self) -> &DiscoveryEventKind {
        &self.kind
    }

    /// Returns the peer-local revision associated with the event.
    #[must_use]
    pub const fn peer_revision(&self) -> DiscoveryRevision {
        self.peer_revision
    }

    /// Returns the operation timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> DiscoveryTimestamp {
        self.timestamp
    }

    pub(crate) const fn sort_rank(&self) -> u8 {
        match self.kind {
            DiscoveryEventKind::AdvertisementReceived { .. } => 0,
            DiscoveryEventKind::Lifecycle { .. } => 1,
            DiscoveryEventKind::Removed { .. } => 2,
        }
    }
}

/// Discovery event payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryEventKind {
    /// A provider observation created or refreshed a registry record.
    AdvertisementReceived {
        /// Previously committed state, or `None` for first discovery.
        previous: Option<DiscoveryState>,
        /// Newly committed state.
        current: DiscoveryState,
        /// Whether the immutable advertisement changed rather than only being observed again.
        advertisement_changed: bool,
    },
    /// A discovered peer changed lifecycle state.
    Lifecycle {
        /// Previously committed state.
        previous: DiscoveryState,
        /// Newly committed state.
        current: DiscoveryState,
    },
    /// A discovered peer was removed from the registry.
    Removed {
        /// Last committed lifecycle state before removal.
        previous: DiscoveryState,
        /// Terminal removal outcome.
        current: DiscoveryState,
    },
}
