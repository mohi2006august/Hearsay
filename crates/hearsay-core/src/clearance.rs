//! The one sanctioned route from the data channel to the instruction channel.
//!
//! `design.md` §4 places [`Declassification`] in `hearsay-policy`. It cannot live
//! there: `Provenanced::declassify` is defined in this crate, and `hearsay-policy`
//! depends on this crate, so referring to a type from `hearsay-policy` here would
//! close a dependency cycle. The witness therefore lives in `hearsay-core`, and
//! the *right to mint one* is what `hearsay-policy` holds, via
//! [`ClearanceAuthority`].
//!
//! # What this actually enforces
//!
//! Rust has no friend modules, so no arrangement of visibility can make a type
//! constructible by exactly one other crate. Being precise about the guarantee
//! matters more than overstating it, because the write-up will rest on it.
//!
//! Enforced by the compiler, with no escape short of `unsafe`:
//!
//! - Data-channel content cannot be passed where instruction-channel content
//!   is expected. Every crossing is a deliberate, visible call.
//! - A [`Declassification`] cannot be forged, copied or reused. It is neither
//!   `Clone` nor `Copy`, and its fields are private with no public constructor.
//! - Only a type carrying `unsafe impl ClearanceAuthority` can mint one.
//!
//! Enforced by lint and audit rather than by types:
//!
//! - That `unsafe impl` appears exactly once outside `#[cfg(test)]`, on
//!   `hearsay-policy::PolicyEngine`. Every crate but this one and `hearsay-policy`
//!   sets `#![forbid(unsafe_code)]`, which makes a second implementation a
//!   compile error rather than a code-review question. `CLAUDE.md` has the
//!   grep that audits this.
//!
//! So the honest claim is: implicit and accidental flows are impossible, and
//! deliberate ones are reduced to a fixed, greppable set of call sites. That is
//! the same shape of guarantee any capability-based design offers, and it is
//! worth stating in those terms rather than as "the compiler prevents
//! laundering".

use crate::ids::{DecisionId, RegionId, RulesetVersion};

/// The right to issue [`Declassification`] witnesses.
///
/// # Safety
///
/// Implementing this trait asserts that the implementor only mints a witness
/// as the result of a deterministic policy decision that has already been
/// written to the decision log. An implementation that mints witnesses on any
/// other basis silently voids the project's central claim — nothing will
/// crash, and the evaluation numbers will be wrong in a way no test detects.
///
/// `hearsay-policy::PolicyEngine` is the only intended implementor.
#[allow(unsafe_code)]
pub unsafe trait ClearanceAuthority {}

/// Proof that the policy engine cleared one specific region under one specific
/// ruleset.
///
/// Deliberately not `Clone` and not `Copy`: a clearance is consumed by the
/// declassification it authorises, so one cannot be reused for a second region.
#[derive(Debug)]
pub struct Declassification {
    decision_id: DecisionId,
    region: RegionId,
    ruleset: RulesetVersion,
}

impl Declassification {
    /// Mint a witness. Requires a [`ClearanceAuthority`], which in this
    /// workspace means `hearsay-policy::PolicyEngine`.
    ///
    /// The authority is taken by reference and unused beyond proving the
    /// caller holds one.
    pub fn mint<A: ClearanceAuthority + ?Sized>(
        _authority: &A,
        decision_id: DecisionId,
        region: RegionId,
        ruleset: RulesetVersion,
    ) -> Self {
        Self {
            decision_id,
            region,
            ruleset,
        }
    }

    /// The decision that issued this clearance.
    pub fn decision_id(&self) -> DecisionId {
        self.decision_id
    }

    /// The region this clearance covers, and only this region.
    pub fn region(&self) -> RegionId {
        self.region
    }

    /// The ruleset version in force when the clearance was issued.
    pub fn ruleset(&self) -> &RulesetVersion {
        &self.ruleset
    }
}
