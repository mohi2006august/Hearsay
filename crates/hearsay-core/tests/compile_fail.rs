//! Compile-fail tests for the provenance boundary.
//!
//! These are the strongest evidence the project has for its central claim:
//! each case in `tests/ui/` is a program that *must not compile*. A passing
//! runtime test shows the defence worked on the inputs we thought of; these
//! show a whole class of bypass is not expressible.
//!
//! # Regenerating expectations
//!
//! trybuild compares compiler output against the `.stderr` file beside each
//! case. On a fresh checkout, or after a rustc upgrade changes the wording of
//! a diagnostic, regenerate them:
//!
//! ```text
//! TRYBUILD=overwrite cargo test -p hearsay-core --test compile_fail
//! ```
//!
//! Then read the diff before committing. A `.stderr` that changed from "no
//! method named `into_inner`" to something else may mean the boundary moved,
//! not that rustc got chattier.

#[test]
fn the_provenance_boundary_cannot_be_bypassed() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
