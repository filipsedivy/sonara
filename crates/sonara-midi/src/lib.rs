//! Music theory, MIDI generators and transforms, the GM drum map and Standard MIDI File import and
//! export, and orchestral instrument metadata (ranges, transposition, articulations).
//!
//! [`form`] plans song sections with musical roles (SON-P10). [`regenerate`] replaces the notes of
//! a bounded region as a validated, undoable patch (SON-P11).
//!
//! The generators reproduce the frozen Python prototype's output exactly (golden data
//! `midi/generators.json`), including its `random.Random` streams ([`pyrandom`]).
#![forbid(unsafe_code)]

pub mod drums;
pub mod form;
pub mod generators;
pub mod gm;
mod num;
pub mod orchestration;
pub mod pyrandom;
pub mod regenerate;
pub mod smf;
pub mod theory;
pub mod transforms;
