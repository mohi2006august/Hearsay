//! A clearance witness has no public constructor.
//!
//! Every field of `Declassification` is private, so there is no struct
//! literal form available outside `hearsay-core`. The only way to obtain one is
//! `Declassification::mint`, which demands a `ClearanceAuthority`.

use hearsay_core::{Declassification, DecisionId, RegionId, RulesetVersion};

fn main() {
    let _forged = Declassification {
        decision_id: DecisionId::new(),
        region: RegionId(0),
        ruleset: RulesetVersion("forged@0000".to_string()),
    };
}
