use crate::{DeviceId, PairingId, PairingRevision, PairingState, PairingTimestamp};

/// Typed Pairing Core event kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingEventKind {
    /// Pairing lifecycle changed.
    Lifecycle {
        /// Previous state, or `None` for creation.
        previous: Option<PairingState>,
        /// Current state, or `None` for removal.
        current: Option<PairingState>,
    },
    /// Trusted-peer record changed.
    Trust {
        /// Previous revocation state, or `None` for insertion.
        previous_revoked: Option<bool>,
        /// Current revocation state, or `None` for removal.
        current_revoked: Option<bool>,
    },
}

/// Immutable Pairing Core event embedded in [`crate::BridgeStateEvent`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingEvent {
    pairing_id: Option<PairingId>,
    device_id: Option<DeviceId>,
    revision: PairingRevision,
    timestamp: PairingTimestamp,
    kind: PairingEventKind,
}

impl PairingEvent {
    pub(crate) fn lifecycle(
        pairing_id: PairingId,
        previous: Option<PairingState>,
        current: Option<PairingState>,
        revision: PairingRevision,
        timestamp: PairingTimestamp,
    ) -> Self {
        Self {
            pairing_id: Some(pairing_id),
            device_id: None,
            revision,
            timestamp,
            kind: PairingEventKind::Lifecycle { previous, current },
        }
    }

    pub(crate) fn trust(
        device_id: DeviceId,
        previous_revoked: Option<bool>,
        current_revoked: Option<bool>,
        revision: PairingRevision,
        timestamp: PairingTimestamp,
    ) -> Self {
        Self {
            pairing_id: None,
            device_id: Some(device_id),
            revision,
            timestamp,
            kind: PairingEventKind::Trust {
                previous_revoked,
                current_revoked,
            },
        }
    }

    /// Returns the pairing identifier for lifecycle events.
    #[must_use]
    pub const fn pairing_id(&self) -> Option<&PairingId> {
        self.pairing_id.as_ref()
    }

    /// Returns the device identifier for trust events.
    #[must_use]
    pub const fn device_id(&self) -> Option<&DeviceId> {
        self.device_id.as_ref()
    }

    /// Returns the affected record revision.
    #[must_use]
    pub const fn revision(&self) -> PairingRevision {
        self.revision
    }

    /// Returns the operation timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> PairingTimestamp {
        self.timestamp
    }

    /// Returns the typed event kind.
    #[must_use]
    pub const fn kind(&self) -> &PairingEventKind {
        &self.kind
    }

    pub(crate) const fn sort_rank(&self) -> u8 {
        match self.kind {
            PairingEventKind::Lifecycle { .. } => 0,
            PairingEventKind::Trust { .. } => 1,
        }
    }
}
