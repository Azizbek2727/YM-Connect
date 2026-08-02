//! Canonical generated YM Connect Protocol Buffer models.
//!
//! The files in `src/gen` are generated from `shared/protocol/proto`. Consumers should import
//! models through [`v1`] rather than including generated files directly.

#![forbid(unsafe_code)]

/// YM Connect protocol major version 1.
pub mod v1 {
    include!("gen/common.rs");
    include!("gen/capabilities.rs");
    include!("gen/errors.rs");
    include!("gen/player.rs");
    include!("gen/control.rs");
    include!("gen/connector.rs");
    include!("gen/session.rs");
}
