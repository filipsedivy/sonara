//! Music theory, MIDI generators and transforms, the GM drum map and Standard MIDI File import and
//! export.
//!
//! The generators reproduce the frozen Python prototype's output exactly (golden data
//! `midi/generators.json`), including its `random.Random` streams ([`pyrandom`]).
#![forbid(unsafe_code)]

pub mod drums;
pub mod generators;
pub mod gm;
mod num;
pub mod pyrandom;
pub mod smf;
pub mod theory;
pub mod transforms;
