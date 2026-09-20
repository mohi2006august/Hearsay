//! There are exactly two channels.
//!
//! `Channel` is sealed, so a downstream crate cannot invent a third with
//! weaker rules — a "semi-trusted" channel that `Provenanced` would then
//! happily carry.

use hearsay_core::{Channel, ChannelLabel};

#[derive(Debug, Clone, Copy)]
struct SemiTrusted;

impl Channel for SemiTrusted {
    const LABEL: ChannelLabel = ChannelLabel::Instruction;
}

fn main() {}
