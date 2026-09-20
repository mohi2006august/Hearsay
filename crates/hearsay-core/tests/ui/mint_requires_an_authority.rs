//! Minting a clearance requires the right to mint one.
//!
//! `ClearanceAuthority` is an unsafe trait, and every crate in the workspace
//! but `hearsay-core` and `hearsay-policy` sets `#![forbid(unsafe_code)]`. So the
//! only way to reach this constructor is to hold the real policy engine —
//! and the only way to add a second authority is an `unsafe impl` that will
//! not compile where it would be needed.

use hearsay_core::{Declassification, DecisionId, RegionId, RulesetVersion};

struct NotAnAuthority;

fn main() {
    let _witness = Declassification::mint(
        &NotAnAuthority,
        DecisionId::new(),
        RegionId(0),
        RulesetVersion("forged@0000".to_string()),
    );
}
