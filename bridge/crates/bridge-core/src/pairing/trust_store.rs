use std::{fmt, sync::Arc};

use crate::{
    BridgeStateStore, DeviceId, PairingError, TrustId, TrustedPeer, TrustedPeerRegistry,
};

/// Runtime-independent read abstraction for authoritative immutable trust records.
///
/// [`BridgeStateStore`] implements this interface directly. Pairing Core writes trust only through
/// Bridge State transactions; later persistence adapters may restore records into Bridge State but
/// must not become a competing mutable source of truth.
pub trait TrustStore: fmt::Debug + Send + Sync {
    /// Looks up one trust record by identifier.
    ///
    /// # Errors
    ///
    /// Returns a structured state error when the authoritative snapshot cannot be read.
    fn trusted_peer(&self, trust_id: &TrustId) -> Result<Option<Arc<TrustedPeer>>, PairingError>;

    /// Looks up the current deterministic trust record associated with one device identifier.
    ///
    /// Active trust takes precedence over historical revoked records. Within the same revocation
    /// class, the latest trust timestamp, revision, and trust identifier define a total ordering.
    ///
    /// # Errors
    ///
    /// Returns a structured state error when the authoritative snapshot cannot be read.
    fn trusted_peer_for_device(
        &self,
        device_id: &DeviceId,
    ) -> Result<Option<Arc<TrustedPeer>>, PairingError>;

    /// Lists trust records in deterministic trust-identifier order.
    ///
    /// # Errors
    ///
    /// Returns a structured state error when the authoritative snapshot cannot be read.
    fn trusted_peers(&self) -> Result<Arc<[TrustedPeer]>, PairingError>;
}

impl TrustStore for BridgeStateStore {
    fn trusted_peer(&self, trust_id: &TrustId) -> Result<Option<Arc<TrustedPeer>>, PairingError> {
        Ok(self.snapshot()?.trusted_peers().get_shared(trust_id))
    }

    fn trusted_peer_for_device(
        &self,
        device_id: &DeviceId,
    ) -> Result<Option<Arc<TrustedPeer>>, PairingError> {
        let snapshot = self.snapshot()?;
        Ok(preferred_trusted_peer(snapshot.trusted_peers(), device_id)
            .and_then(|peer| snapshot.trusted_peers().get_shared(peer.trust_id())))
    }

    fn trusted_peers(&self) -> Result<Arc<[TrustedPeer]>, PairingError> {
        Ok(self
            .snapshot()?
            .trusted_peers()
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .into())
    }
}

pub(crate) fn preferred_trusted_peer<'a>(
    registry: &'a TrustedPeerRegistry,
    device_id: &DeviceId,
) -> Option<&'a TrustedPeer> {
    registry
        .values()
        .filter(|peer| peer.peer().device_id == device_id.as_str())
        .max_by(|left, right| {
            left.revocation()
                .is_revoked()
                .cmp(&right.revocation().is_revoked())
                .reverse()
                .then_with(|| left.trusted_at().cmp(&right.trusted_at()))
                .then_with(|| left.revision().cmp(&right.revision()))
                .then_with(|| left.trust_id().cmp(right.trust_id()))
        })
}
