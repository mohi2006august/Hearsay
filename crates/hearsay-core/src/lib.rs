//! Domain types for the cross-modal injection defence.
//!
//! This crate has no I/O, no async runtime and no dependency on any other
//! workspace crate. That is deliberate: the provenance invariant in
//! [`provenance`] is the project's central claim, and keeping it in a leaf
//! crate means it can be property-tested and compile-fail-tested without
//! standing up a server.
//!
//! # The invariant
//!
//! > Content that entered through a data channel cannot reach the upstream
//! > instruction context except through a [`Declassification`] issued by the
//! > policy engine.
//!
//! See [`provenance`] for how the type system carries this, and
//! [`clearance`] for the one escape hatch and why it is `unsafe`.
//!
//! [`Declassification`]: clearance::Declassification

// `unsafe_code` is denied rather than forbidden because this crate *declares*
// the `ClearanceAuthority` unsafe trait. It never implements it. The single
// `#[allow]` sits on that declaration in `clearance.rs` and nowhere else.
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod clearance;
pub mod decision;
pub mod ids;
pub mod provenance;
pub mod region;
pub mod text;

pub use clearance::{ClearanceAuthority, Declassification};
pub use decision::{RegionAction, RequestOutcome, Score};
pub use ids::{
    ContentHash, DecisionId, EngineId, ModelVersion, RegionId, RequestId, RulesetVersion,
};
pub use provenance::{
    Channel, ChannelLabel, Data, Instruction, Origin, ProvenanceError, Provenanced,
};
pub use region::{BBox, DegradeReason, Extraction, ExtractionOutcome, TextRegion};
pub use text::{NormalizedText, TextError};
