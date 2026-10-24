//! Markers and arrangement sections (`sonara.domain.markers`).

use super::{has_text, validate_color};
use crate::EPSILON;
use crate::error::{Result, require, require_finite};
use crate::ids::{self, IdGenerator};

/// A named position on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    /// Stable ID (`mrk_…`).
    pub id: String,
    /// Position in beats (≥ 0).
    pub beat: f64,
    /// Display name.
    pub name: String,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
}

impl Marker {
    /// A validated marker.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for a non-finite or negative beat or an
    /// invalid color.
    pub fn new(
        id: impl Into<String>,
        beat: f64,
        name: impl Into<String>,
        color: Option<String>,
    ) -> Result<Self> {
        require_finite(beat, "marker beat")?;
        require(beat >= 0.0, || "marker beat must be >= 0".to_owned())?;
        validate_color(color.as_deref())?;
        Ok(Self {
            id: id.into(),
            beat,
            name: name.into(),
            color,
        })
    }

    /// A new marker with a fresh ID.
    ///
    /// # Errors
    ///
    /// The validation errors of [`Marker::new`].
    pub fn create(ids: &mut dyn IdGenerator, beat: f64, name: impl Into<String>) -> Result<Self> {
        Self::new(ids.new_id(ids::MARKER), beat, name, None)
    }
}

/// A named span of the song structure (intro, verse, drop, …).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Stable ID (`sec_…`).
    pub id: String,
    /// Display name (not only whitespace).
    pub name: String,
    /// Start in beats (≥ 0).
    pub start: f64,
    /// Length in beats (> 0).
    pub length: f64,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
}

impl Section {
    /// A validated section.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] with the prototype's messages.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        start: f64,
        length: f64,
        color: Option<String>,
    ) -> Result<Self> {
        let name = name.into();
        require_finite(start, "section start")?;
        require_finite(length, "section length")?;
        require(start >= 0.0, || "section start must be >= 0".to_owned())?;
        require(length > EPSILON, || "section length must be > 0".to_owned())?;
        require(has_text(&name), || {
            "section name must not be empty".to_owned()
        })?;
        validate_color(color.as_deref())?;
        Ok(Self {
            id: id.into(),
            name,
            start,
            length,
            color,
        })
    }

    /// A new section with a fresh ID.
    ///
    /// # Errors
    ///
    /// The validation errors of [`Section::new`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        name: impl Into<String>,
        start: f64,
        length: f64,
    ) -> Result<Self> {
        Self::new(ids.new_id(ids::SECTION), name, start, length, None)
    }

    /// End in beats.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.start + self.length
    }
}

#[cfg(test)]
mod tests {
    use super::{Marker, Section};

    #[test]
    fn rules_follow_the_prototype() {
        let err = Marker::new("mrk_1", -1.0, "Drop", None).expect_err("negative beat");
        assert_eq!(err.message, "marker beat must be >= 0");
        let err = Section::new("sec_1", "  ", 0.0, 4.0, None).expect_err("blank name");
        assert_eq!(err.message, "section name must not be empty");
        let section = Section::new("sec_1", "Verse", 4.0, 8.0, None).expect("valid");
        assert!((section.end() - 12.0).abs() < f64::EPSILON);
    }
}
