//! The deterministic policy engine.
//!
//! Combines a channel label and a classifier score into a decision. No model
//! call, no I/O, no async — `design.md` §6.5. The engine is a concrete type
//! rather than a trait because it must not be pluggable: a swappable policy
//! would make the audit log's ruleset stamp meaningless.
//!
//! This crate holds the workspace's only `unsafe impl`, which grants
//! [`PolicyEngine`] the right to mint [`hearsay_core::Declassification`]
//! witnesses. See [`hearsay_core::clearance`] for the precise scope of what that
//! buys.

// Denied rather than forbidden: this crate carries exactly one documented
// `unsafe impl ClearanceAuthority for PolicyEngine`, in `engine.rs`.
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod engine;
pub mod ruleset;

pub use engine::{
    region_inputs, PolicyEngine, Redactable, RegionInput, RegionVerdict, RequestVerdict, RuleId,
    UnknownRuleId,
};
pub use ruleset::{Ruleset, RulesetError};
