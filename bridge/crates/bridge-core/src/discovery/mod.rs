//! Runtime-independent discovery contracts, immutable advertisements, peer lifecycle, filtering,
//! and Bridge State orchestration.
//!
//! Discovery Core contains no provider implementation, socket, platform API, browser integration,
//! or asynchronous runtime dependency. Future providers own their I/O and authenticity checks;
//! [`DiscoveryManager`] remains the only discovery registry mutation authority.

mod error;
mod event;
mod manager;
mod model;
mod provider;

pub use error::{DiscoveryError, DiscoveryOperation, DiscoveryResult};
pub use event::{DiscoveryEvent, DiscoveryEventKind};
pub use manager::{
    DiscoveryManager, DiscoveryMutation, ExpireDiscoveredPeers, ExpiredDiscoveries,
    ReceiveDiscoveryAdvertisement, RemoveDiscoveredPeer, TransitionDiscoveredPeer,
    ValidateDiscoveredPeer,
};
pub use model::{
    AdvertisementSignatureMetadata, DEFAULT_MAXIMUM_ADVERTISEMENT_LIFETIME_MS,
    DEFAULT_MAXIMUM_FUTURE_CLOCK_SKEW_MS, DEFAULT_MAXIMUM_METADATA_BYTES,
    DEFAULT_MAXIMUM_METADATA_ENTRIES, DiscoveredPeer, DiscoveryAdvertisement,
    DiscoveryCapabilities, DiscoveryFilter, DiscoveryModelError, DiscoveryPeerKey, DiscoveryPolicy,
    DiscoveryRevision, DiscoverySnapshot, DiscoverySource, DiscoveryState, DiscoveryTimestamp,
};
pub use provider::{DiscoveryFuture, DiscoveryProvider};

pub(crate) use model::version_key;

#[cfg(test)]
mod tests;
