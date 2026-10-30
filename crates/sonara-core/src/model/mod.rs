//! The project model (`sonara.domain`): tracks, clips, devices, automation, media, markers and
//! the project aggregate.
//!
//! Small value objects ([`crate::midi::MidiNote`], [`clip::Fade`], [`track::Send`], …) have
//! public fields and a validating `new`. Aggregates whose invariants span several fields or keep
//! their contents sorted are [`Valid`] values: the plain data struct is checked and normalized
//! once by [`Valid::new`], shared through an [`Arc`], and changed copy-on-write with
//! [`Valid::edit`], like the prototype's frozen dataclasses and `dataclasses.replace`. Cloning a
//! project is cheap, which keeps undo snapshots cheap.

pub mod automation;
pub mod clip;
pub mod device;
pub mod markers;
pub mod media;
pub mod part;
pub mod project;
pub mod track;

use std::cmp::Ordering;
use std::ops::Deref;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{Result, require};
use crate::pyfmt::str_repr;

/// A JSON object whose keys keep their insertion order, for open-ended data such as device
/// state, editor state and unknown keys preserved from newer files (`extra`).
pub type JsonMap = serde_json::Map<String, Value>;

/// Data whose invariants are checked, and whose contents are normalized, in one place.
pub trait Validate: Clone {
    /// Checks the invariants and normalizes the data in place (for example sorts its clips).
    ///
    /// # Errors
    ///
    /// The prototype's [`crate::error::DomainError`] for the first violated rule.
    fn validate(&mut self) -> Result<()>;
}

/// Validated, immutable, cheaply cloned data: the plain data struct behind an [`Arc`].
#[derive(Debug, PartialEq)]
pub struct Valid<T>(Arc<T>);

impl<T> Clone for Valid<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Validate> Valid<T> {
    /// Validates and normalizes `data`.
    ///
    /// # Errors
    ///
    /// The error of [`Validate::validate`].
    pub fn new(mut data: T) -> Result<Self> {
        data.validate()?;
        Ok(Self(Arc::new(data)))
    }

    /// A changed copy, validated again (the prototype's `dataclasses.replace`).
    ///
    /// # Errors
    ///
    /// The error of [`Validate::validate`] for the changed data.
    pub fn edit(&self, change: impl FnOnce(&mut T)) -> Result<Self> {
        let mut data = self.to_data();
        change(&mut data);
        Self::new(data)
    }

    /// A copy changed by a fallible function, validated again.
    ///
    /// # Errors
    ///
    /// The error of `change`, or of [`Validate::validate`] for the changed data.
    pub fn try_edit(&self, change: impl FnOnce(&mut T) -> Result<()>) -> Result<Self> {
        let mut data = self.to_data();
        change(&mut data)?;
        Self::new(data)
    }

    /// A copy of the plain data.
    #[must_use]
    pub fn to_data(&self) -> T {
        T::clone(&self.0)
    }
}

impl<T> Valid<T> {
    /// Whether both values share the same allocation (an unchanged part of an edited project).
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Deref for Valid<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

/// Orders floats like Python's `<` on validated (finite) values: `-0.0` equals `0.0`, so a
/// stable sort keeps their original order.
#[must_use]
pub fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

/// Python's float `%`: the result has the sign of the divisor.
#[must_use]
pub fn py_mod(value: f64, divisor: f64) -> f64 {
    let rem = value % divisor;
    if rem != 0.0 && (rem < 0.0) != (divisor < 0.0) {
        rem + divisor
    } else {
        rem
    }
}

/// Colors are `#rrggbb` strings, or `None` for an automatic color.
///
/// # Errors
///
/// [`crate::error::ErrorCode::InvalidArgument`]: `color must be '#rrggbb', got '…'`.
pub fn validate_color(color: Option<&str>) -> Result<()> {
    let Some(color) = color else {
        return Ok(());
    };
    let valid = color.chars().count() == 7
        && color.starts_with('#')
        && color.chars().skip(1).all(|c| c.is_ascii_hexdigit());
    require(valid, || {
        format!("color must be '#rrggbb', got {}", str_repr(color))
    })
}

/// Whether `text` has a character other than whitespace (Python's `bool(text.strip())`).
pub(crate) fn has_text(text: &str) -> bool {
    !text.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::validate_color;

    #[test]
    fn colors_follow_the_prototype() {
        assert!(validate_color(None).is_ok());
        assert!(validate_color(Some("#A0b1C2")).is_ok());
        let err = validate_color(Some("red")).expect_err("not a hex color");
        assert_eq!(err.message, "color must be '#rrggbb', got 'red'");
        assert!(validate_color(Some("#12345g")).is_err());
    }
}
