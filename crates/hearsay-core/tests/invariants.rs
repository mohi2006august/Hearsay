//! Property tests for the invariants the compile-fail suite cannot express.
//!
//! The `ui/` cases prove certain programs do not exist. These prove the
//! programs that *do* exist behave as claimed for arbitrary payloads.

use hearsay_core::{ChannelLabel, ContentHash, NormalizedText, Origin, Provenanced, RegionId};
use proptest::prelude::*;

fn any_data_origin() -> impl Strategy<Value = Origin> {
    prop_oneof![
        Just(Origin::UploadedImage {
            hash: ContentHash::from_bytes([0u8; 32]),
            part_index: 0,
        }),
        Just(Origin::FetchedUrl {
            url: "https://example.test/a.png".to_string(),
            hash: ContentHash::from_bytes([1u8; 32]),
        }),
        Just(Origin::ToolOutput {
            tool: "browser".to_string(),
        }),
    ]
}

proptest! {
    /// No payload can talk its way onto the instruction channel at
    /// construction. This is the runtime half of `data_is_not_an_instruction`.
    #[test]
    fn observed_content_is_always_data_channel(
        payload in ".*",
        origin in any_data_origin(),
    ) {
        let p = Provenanced::observed(payload.clone(), origin)
            .expect("every generated origin is data-bearing");
        prop_assert_eq!(p.label(), ChannelLabel::Data);
        prop_assert_eq!(p.inspect().as_str(), payload.as_str());
    }

    /// `map` is the one transform available to pipeline stages, so it must
    /// not be a laundering route.
    #[test]
    fn map_preserves_channel_and_origin(
        payload in ".*",
        origin in any_data_origin(),
    ) {
        let p = Provenanced::observed(payload, origin.clone())
            .expect("data-bearing origin");
        let mapped = p.map(|s: String| s.len());
        prop_assert_eq!(mapped.label(), ChannelLabel::Data);
        prop_assert_eq!(mapped.origin(), &origin);
    }

    /// The user's own prompt is never reclassified as observed data, whatever
    /// it contains. Getting this wrong would subject the user's instructions
    /// to redaction — a benign-utility failure, not a security one, and the
    /// kind that is easy to ship unnoticed.
    #[test]
    fn the_user_prompt_is_never_data_bearing(payload in ".*") {
        prop_assert!(Provenanced::observed(payload, Origin::UserPrompt).is_err());
    }

    /// Identity normalisation must always produce a valid offset map,
    /// including for multi-byte UTF-8, which is exactly where an off-by-one
    /// would send a redaction to the wrong pixels.
    #[test]
    fn identity_offset_maps_always_validate(text in ".*") {
        let t = NormalizedText::identity(text.clone(), RegionId(0));
        prop_assert_eq!(t.as_str(), text.as_str());

        let offsets: Vec<u32> = (0..text.len())
            .map(|i| u32::try_from(i).expect("bounded by proptest input size"))
            .collect();
        prop_assert!(NormalizedText::new(text, offsets, RegionId(0)).is_ok());
    }

    /// An offset map of the wrong length is always rejected. Byte length, not
    /// character count — the distinction matters for any non-ASCII input.
    #[test]
    fn offset_maps_of_the_wrong_length_are_always_rejected(
        text in "\\PC{1,64}",
        extra in 1usize..8,
    ) {
        let offsets: Vec<u32> = (0..text.len() + extra)
            .map(|i| u32::try_from(i).expect("bounded"))
            .collect();
        prop_assert!(NormalizedText::new(text, offsets, RegionId(0)).is_err());
    }
}
