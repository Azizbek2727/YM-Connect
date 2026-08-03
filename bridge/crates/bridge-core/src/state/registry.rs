use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::Arc,
};

use ym_connect_protocol::v1::{
    BrowserDescriptor, CapabilitySet, DeviceDescriptor, SessionEstablished,
};

use super::{
    CapabilityOwner, ConnectorId, DeviceId, SessionId, StateIdentifierError,
};

/// Registry contained by a Bridge state snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryKind {
    /// Active session registry.
    Sessions,
    /// Known device registry.
    Devices,
    /// Browser connector registry.
    Connectors,
    /// Transport connection registry.
    Connections,
    /// Capability ownership registry.
    Capabilities,
}

impl fmt::Display for RegistryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Sessions => "sessions",
            Self::Devices => "devices",
            Self::Connectors => "connectors",
            Self::Connections => "connections",
            Self::Capabilities => "capabilities",
        })
    }
}

/// Registry operation that failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryOperation {
    /// Insert a previously absent entry.
    Insert,
    /// Replace an existing entry.
    Replace,
    /// Insert or replace an entry.
    Upsert,
    /// Remove an existing entry.
    Remove,
}

impl fmt::Display for RegistryOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Insert => "insert",
            Self::Replace => "replace",
            Self::Upsert => "upsert",
            Self::Remove => "remove",
        })
    }
}

/// Structured registry failure reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryFailure {
    /// The canonical record did not contain a valid identifier.
    InvalidIdentifier,
    /// An insertion targeted an existing key.
    DuplicateKey,
    /// A replacement or removal targeted a missing key.
    MissingKey,
}

/// Structured state-registry operation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryStateError {
    registry: RegistryKind,
    operation: RegistryOperation,
    failure: RegistryFailure,
    key: Option<String>,
    identifier: Option<StateIdentifierError>,
}

impl RegistryStateError {
    fn invalid_identifier(
        registry: RegistryKind,
        operation: RegistryOperation,
        identifier: StateIdentifierError,
    ) -> Self {
        Self {
            registry,
            operation,
            failure: RegistryFailure::InvalidIdentifier,
            key: None,
            identifier: Some(identifier),
        }
    }

    fn key_failure(
        registry: RegistryKind,
        operation: RegistryOperation,
        failure: RegistryFailure,
        key: &impl ToString,
    ) -> Self {
        Self {
            registry,
            operation,
            failure,
            key: Some(key.to_string()),
            identifier: None,
        }
    }

    /// Returns the affected registry.
    #[must_use]
    pub const fn registry(&self) -> RegistryKind {
        self.registry
    }

    /// Returns the failed operation.
    #[must_use]
    pub const fn operation(&self) -> RegistryOperation {
        self.operation
    }

    /// Returns the structured failure reason.
    #[must_use]
    pub const fn failure(&self) -> RegistryFailure {
        self.failure
    }

    /// Returns the affected key when one was available.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
}

impl fmt::Display for RegistryStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.key, &self.identifier, self.failure) {
            (_, Some(identifier), RegistryFailure::InvalidIdentifier) => write!(
                formatter,
                "cannot {} {} registry entry: {identifier}",
                self.operation, self.registry
            ),
            (Some(key), _, RegistryFailure::DuplicateKey) => write!(
                formatter,
                "cannot {} {} registry entry {key:?}: key already exists",
                self.operation, self.registry
            ),
            (Some(key), _, RegistryFailure::MissingKey) => write!(
                formatter,
                "cannot {} {} registry entry {key:?}: key does not exist",
                self.operation, self.registry
            ),
            _ => write!(
                formatter,
                "cannot {} {} registry entry",
                self.operation, self.registry
            ),
        }
    }
}

impl Error for RegistryStateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.identifier
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Result of a successful registry mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryMutation<K> {
    /// A new entry was inserted.
    Inserted(K),
    /// An existing entry changed.
    Replaced(K),
    /// An existing entry was removed.
    Removed(K),
    /// The requested value was already present.
    Unchanged(K),
}

impl<K> RegistryMutation<K> {
    /// Returns whether the registry content changed.
    #[must_use]
    pub const fn changed(&self) -> bool {
        !matches!(self, Self::Unchanged(_))
    }
}

/// Canonical record stored by a deterministic state registry.
pub trait StateRegistryValue: Clone + fmt::Debug + PartialEq + Send + Sync + 'static {
    /// Stable registry key type.
    type Key: Clone + fmt::Debug + fmt::Display + Ord + Send + Sync + 'static;

    /// Registry containing this value type.
    const REGISTRY_KIND: RegistryKind;

    /// Extracts and validates the canonical key.
    ///
    /// # Errors
    ///
    /// Returns [`StateIdentifierError`] when the record does not contain a valid key.
    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError>;
}

#[derive(Clone, Copy)]
enum WriteOperation {
    Insert,
    Replace,
    Upsert,
}

impl WriteOperation {
    const fn public(self) -> RegistryOperation {
        match self {
            Self::Insert => RegistryOperation::Insert,
            Self::Replace => RegistryOperation::Replace,
            Self::Upsert => RegistryOperation::Upsert,
        }
    }
}

/// Deterministically ordered, copy-on-write registry used by Bridge snapshots.
#[derive(Clone, Debug, PartialEq)]
pub struct StateRegistry<V>
where
    V: StateRegistryValue,
{
    entries: BTreeMap<V::Key, Arc<V>>,
}

impl<V> StateRegistry<V>
where
    V: StateRegistryValue,
{
    /// Creates an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns whether `key` is registered.
    #[must_use]
    pub fn contains_key(&self, key: &V::Key) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns a registered value.
    #[must_use]
    pub fn get(&self, key: &V::Key) -> Option<&V> {
        self.entries.get(key).map(AsRef::as_ref)
    }

    /// Returns a shared registered value.
    #[must_use]
    pub fn get_shared(&self, key: &V::Key) -> Option<Arc<V>> {
        self.entries.get(key).cloned()
    }

    /// Iterates entries in stable key order.
    #[must_use]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&V::Key, &V)> + ExactSizeIterator {
        self.entries
            .iter()
            .map(|(key, value)| (key, value.as_ref()))
    }

    /// Iterates keys in stable order.
    #[must_use]
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &V::Key> + ExactSizeIterator {
        self.entries.keys()
    }

    /// Iterates values in stable key order.
    #[must_use]
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.entries.values().map(AsRef::as_ref)
    }

    /// Inserts a new canonical value.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryStateError`] when the record identifier is invalid or already exists.
    pub fn insert(&mut self, value: V) -> Result<RegistryMutation<V::Key>, RegistryStateError> {
        self.write(value, WriteOperation::Insert)
    }

    /// Replaces an existing canonical value.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryStateError`] when the record identifier is invalid or does not exist.
    pub fn replace(&mut self, value: V) -> Result<RegistryMutation<V::Key>, RegistryStateError> {
        self.write(value, WriteOperation::Replace)
    }

    /// Inserts or replaces a canonical value.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryStateError`] when the record identifier is invalid.
    pub fn upsert(&mut self, value: V) -> Result<RegistryMutation<V::Key>, RegistryStateError> {
        self.write(value, WriteOperation::Upsert)
    }

    /// Removes an existing value.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryStateError`] when `key` does not exist.
    pub fn remove(
        &mut self,
        key: &V::Key,
    ) -> Result<(RegistryMutation<V::Key>, Arc<V>), RegistryStateError> {
        self.entries.remove(key).map_or_else(
            || {
                Err(RegistryStateError::key_failure(
                    V::REGISTRY_KIND,
                    RegistryOperation::Remove,
                    RegistryFailure::MissingKey,
                    key,
                ))
            },
            |value| Ok((RegistryMutation::Removed(key.clone()), value)),
        )
    }

    /// Removes a value when present.
    #[must_use]
    pub fn remove_if_present(&mut self, key: &V::Key) -> Option<Arc<V>> {
        self.entries.remove(key)
    }

    /// Removes all values and returns the number removed.
    #[must_use]
    pub fn clear(&mut self) -> usize {
        let removed = self.entries.len();
        self.entries.clear();
        removed
    }

    fn write(
        &mut self,
        value: V,
        operation: WriteOperation,
    ) -> Result<RegistryMutation<V::Key>, RegistryStateError> {
        let public_operation = operation.public();
        let key = value.registry_key().map_err(|identifier| {
            RegistryStateError::invalid_identifier(
                V::REGISTRY_KIND,
                public_operation,
                identifier,
            )
        })?;
        let existing = self.entries.get(&key);

        if matches!(operation, WriteOperation::Insert) && existing.is_some() {
            return Err(RegistryStateError::key_failure(
                V::REGISTRY_KIND,
                public_operation,
                RegistryFailure::DuplicateKey,
                &key,
            ));
        }
        if matches!(operation, WriteOperation::Replace) && existing.is_none() {
            return Err(RegistryStateError::key_failure(
                V::REGISTRY_KIND,
                public_operation,
                RegistryFailure::MissingKey,
                &key,
            ));
        }
        if existing.is_some_and(|existing| existing.as_ref() == &value) {
            return Ok(RegistryMutation::Unchanged(key));
        }

        let mutation = if existing.is_some() {
            RegistryMutation::Replaced(key.clone())
        } else {
            RegistryMutation::Inserted(key.clone())
        };
        self.entries.insert(key, Arc::new(value));
        Ok(mutation)
    }
}

impl<V> Default for StateRegistry<V>
where
    V: StateRegistryValue,
{
    fn default() -> Self {
        Self::new()
    }
}

impl StateRegistryValue for SessionEstablished {
    type Key = SessionId;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Sessions;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        SessionId::new(self.session_id.clone())
    }
}

impl StateRegistryValue for DeviceDescriptor {
    type Key = DeviceId;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Devices;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        DeviceId::new(self.device_id.clone())
    }
}

impl StateRegistryValue for BrowserDescriptor {
    type Key = ConnectorId;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Connectors;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        ConnectorId::new(self.connector_id.clone())
    }
}

/// Runtime ownership association for a canonical Protocol Buffer capability set.
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityRegistration {
    owner: CapabilityOwner,
    capabilities: CapabilitySet,
}

impl CapabilityRegistration {
    /// Creates a capability registration.
    #[must_use]
    pub fn new(owner: CapabilityOwner, capabilities: CapabilitySet) -> Self {
        Self {
            owner,
            capabilities,
        }
    }

    /// Returns the capability owner.
    #[must_use]
    pub const fn owner(&self) -> &CapabilityOwner {
        &self.owner
    }

    /// Returns the canonical capability set.
    #[must_use]
    pub const fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

impl StateRegistryValue for CapabilityRegistration {
    type Key = CapabilityOwner;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Capabilities;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        Ok(self.owner.clone())
    }
}
