use std::{future::Future, pin::Pin};

use crate::{DiscoveryAdvertisement, DiscoveryFilter, DiscoveryResult, DiscoverySource};

/// Boxed runtime-neutral future returned by a discovery provider.
pub type DiscoveryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Runtime-independent provider contract implemented by future discovery adapters.
///
/// Provider implementations own their networking or platform integration. They do not own the
/// canonical discovery registry. A runtime orchestrator receives advertisements from a provider,
/// performs provider-specific authenticity validation, and submits accepted observations to
/// [`crate::DiscoveryManager`].
pub trait DiscoveryProvider: Send + Sync {
    /// Returns the stable source identifier represented by this provider.
    fn source(&self) -> &DiscoverySource;

    /// Starts provider-specific discovery using the supplied deterministic filter.
    fn start<'a>(&'a self, filter: &'a DiscoveryFilter)
    -> DiscoveryFuture<'a, DiscoveryResult<()>>;

    /// Returns the next immutable advertisement, or `None` when no item is currently available.
    fn next_advertisement(
        &self,
    ) -> DiscoveryFuture<'_, DiscoveryResult<Option<DiscoveryAdvertisement>>>;

    /// Performs provider-specific advertisement authenticity validation.
    fn validate_advertisement<'a>(
        &'a self,
        advertisement: &'a DiscoveryAdvertisement,
    ) -> DiscoveryFuture<'a, DiscoveryResult<()>>;

    /// Stops provider-specific discovery and releases provider-owned resources.
    fn stop(&self) -> DiscoveryFuture<'_, DiscoveryResult<()>>;
}
