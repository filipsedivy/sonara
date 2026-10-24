//! The Broadcast WAV `bext` chunk (EBU Tech 3285 v2), as the prototype's `write_bext` and
//! `read_bext` (`audio_export.py`) insert and read it: the chunk goes right before `data`, and
//! the RIFF size is patched.
//!
//! Layout (602 bytes before the coding history): description (256, ASCII with `?` for other
//! characters, NUL-padded), originator (32), originator reference (32), origination date (10,
//! `%Y-%m-%d`) and time (8, `%H:%M:%S`) of the clock in UTC, time reference as two
//! little-endian `u32` (low, high), version 2, 64 zero UMID bytes, loudness value, range, max
//! true peak, max momentary and max short-term loudness as `i16` hundredths (0x7FFF when
//! missing), 180 reserved bytes, then the coding history; the body is padded to an even length.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use jiff::Timestamp;
use jiff::tz::Offset;
use serde_json::{Map, Value};
use sonara_dsp::analysis::AnalysisReport;

use super::ExportError;

/// The loudness fields of a `bext` chunk (`None` is stored as 0x7FFF).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BextLoudness {
    pub(crate) integrated: Option<f64>,
    pub(crate) range: Option<f64>,
    pub(crate) true_peak: Option<f64>,
    pub(crate) momentary_max: Option<f64>,
    pub(crate) short_term_max: Option<f64>,
}

impl BextLoudness {
    /// The fields from an analysis of the written file.
    pub(crate) fn from_report(report: &AnalysisReport) -> Self {
        Self {
            integrated: Some(report.integrated_lufs),
            range: Some(report.loudness_range_lu),
            true_peak: Some(report.true_peak_dbtp),
            momentary_max: Some(report.momentary_max_lufs),
            short_term_max: Some(report.short_term_max_lufs),
        }
    }
}

/// What [`read_bext`] reports, the prototype's `read_bext` dictionary.
#[derive(Debug, Clone, PartialEq)]
pub struct BextInfo {
    /// The description, up to its first NUL.
    pub description: String,
    /// The originator.
    pub originator: String,
    /// The first sample's position in samples since midnight.
    pub time_reference: u64,
    /// The `bext` version.
    pub version: u16,
    /// Integrated loudness in LUFS.
    pub loudness_value: Option<f64>,
    /// Loudness range in LU.
    pub loudness_range: Option<f64>,
    /// Maximum true peak in dBTP.
    pub max_true_peak: Option<f64>,
}

impl BextInfo {
    /// The prototype's dictionary, in its key order.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("description".into(), Value::from(self.description.clone()));
        m.insert("originator".into(), Value::from(self.originator.clone()));
        m.insert("time_reference".into(), Value::from(self.time_reference));
        m.insert("version".into(), Value::from(self.version));
        m.insert(
            "loudness_value".into(),
            self.loudness_value.map_or(Value::Null, Value::from),
        );
        m.insert(
            "loudness_range".into(),
            self.loudness_range.map_or(Value::Null, Value::from),
        );
        m.insert(
            "max_true_peak".into(),
            self.max_true_peak.map_or(Value::Null, Value::from),
        );
        Value::Object(m)
    }
}

/// `text.encode("ascii", "replace")[:n].ljust(n, b"\0")`.
fn fixed(text: &str, n: usize) -> Vec<u8> {
    let mut out: Vec<u8> = text
        .chars()
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .take(n)
        .collect();
    out.resize(n, 0);
    out
}

/// `round(v * 100)` clamped to `i16`, 0x7FFF for a missing or non-finite value.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and clamped to the i16 range first"
)]
fn hundredths(v: Option<f64>) -> i16 {
    match v {
        Some(v) if v.is_finite() => (v * 100.0).round_ties_even().clamp(-32768.0, 32767.0) as i16,
        _ => 0x7FFF,
    }
}

/// `datetime.now(UTC).strftime("%Y-%m-%d")` and `"%H:%M:%S"` of `time`.
fn date_and_time(time: SystemTime) -> (String, String) {
    let timestamp = Timestamp::try_from(time).unwrap_or(if time < UNIX_EPOCH {
        Timestamp::MIN
    } else {
        Timestamp::MAX
    });
    let t = Offset::UTC.to_datetime(timestamp);
    (
        format!("{:04}-{:02}-{:02}", t.year(), t.month(), t.day()),
        format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second()),
    )
}

/// The chunk (`bext`, size, body).
pub(crate) fn bext_chunk(
    description: &str,
    originator: &str,
    time_reference: u64,
    loudness: Option<&BextLoudness>,
    coding_history: &str,
    now: SystemTime,
) -> Vec<u8> {
    let (date, time) = date_and_time(now);
    let mut body = Vec::with_capacity(640);
    body.extend_from_slice(&fixed(description, 256));
    body.extend_from_slice(&fixed(originator, 32));
    body.extend_from_slice(&fixed("", 32));
    body.extend_from_slice(&fixed(&date, 10));
    body.extend_from_slice(&fixed(&time, 8));
    let low = u32::try_from(time_reference & 0xFFFF_FFFF).unwrap_or_default();
    let high = u32::try_from(time_reference >> 32).unwrap_or(u32::MAX);
    body.extend_from_slice(&low.to_le_bytes());
    body.extend_from_slice(&high.to_le_bytes());
    body.extend_from_slice(&2u16.to_le_bytes());
    body.resize(body.len() + 64, 0);
    let fields = [
        loudness.and_then(|l| l.integrated),
        loudness.and_then(|l| l.range),
        loudness.and_then(|l| l.true_peak),
        loudness.and_then(|l| l.momentary_max),
        loudness.and_then(|l| l.short_term_max),
    ];
    for v in fields {
        body.extend_from_slice(&hundredths(v).to_le_bytes());
    }
    body.resize(body.len() + 180, 0);
    body.extend(
        coding_history
            .chars()
            .map(|c| if c.is_ascii() { c as u8 } else { b'?' }),
    );
    if body.len() % 2 == 1 {
        body.push(0);
    }
    let mut chunk = Vec::with_capacity(body.len() + 8);
    chunk.extend_from_slice(b"bext");
    chunk.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_le_bytes());
    chunk.extend_from_slice(&body);
    chunk
}

/// The RIFF chunks after the `WAVE` id: `(offset, id, size)`.
fn chunks(data: &[u8]) -> impl Iterator<Item = (usize, [u8; 4], usize)> + '_ {
    let mut pos = 12usize;
    std::iter::from_fn(move || {
        let id: [u8; 4] = data.get(pos..pos + 4)?.try_into().ok()?;
        let size: [u8; 4] = data.get(pos + 4..pos + 8)?.try_into().ok()?;
        let size = usize::try_from(u32::from_le_bytes(size)).ok()?;
        let here = pos;
        pos = pos.checked_add(8 + size + (size & 1))?;
        Some((here, id, size))
    })
}

/// `write_bext`: inserts `chunk` before the `data` chunk of the RIFF WAVE file at `path`,
/// patches the RIFF size, and replaces the file through `<path>.bext.tmp`.
///
/// # Errors
///
/// [`ExportError::Format`] when the file is not RIFF WAVE or has no `data` chunk, and
/// [`ExportError::Io`] when reading or writing fails.
pub(crate) fn insert_bext(path: &Path, chunk: &[u8]) -> Result<(), ExportError> {
    let data = fs::read(path).map_err(|e| ExportError::io("read", path, e))?;
    if data.get(..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WAVE") {
        return Err(ExportError::Format(
            "BWF requires a RIFF/WAVE file".to_owned(),
        ));
    }
    let pos = chunks(&data)
        .find(|(_, id, _)| id == b"data")
        .map(|(pos, _, _)| pos)
        .ok_or_else(|| ExportError::Format("WAV file has no data chunk".to_owned()))?;
    let mut out = Vec::with_capacity(data.len() + chunk.len());
    out.extend_from_slice(data.get(..pos).unwrap_or_default());
    out.extend_from_slice(chunk);
    out.extend_from_slice(data.get(pos..).unwrap_or_default());
    let riff = u32::try_from(out.len() - 8).unwrap_or(u32::MAX);
    if let Some(slot) = out.get_mut(4..8) {
        slot.copy_from_slice(&riff.to_le_bytes());
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".bext.tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, &out).map_err(|e| ExportError::io("write", &tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| ExportError::io("replace", path, e))
}

/// Reads the `bext` chunk of a RIFF WAVE file, as the prototype's `read_bext`; `None` when
/// the file has none.
///
/// # Errors
///
/// [`ExportError::Io`] when the file cannot be read, and [`ExportError::Format`] for a `bext`
/// chunk too short for its fields.
pub fn read_bext(path: &Path) -> Result<Option<BextInfo>, ExportError> {
    let data = fs::read(path).map_err(|e| ExportError::io("read", path, e))?;
    let Some((pos, _, size)) = chunks(&data).find(|(_, id, _)| id == b"bext") else {
        return Ok(None);
    };
    let b = data
        .get(pos + 8..pos + 8 + size)
        .filter(|b| b.len() >= 422)
        .ok_or_else(|| ExportError::Format("bext chunk is too short".to_owned()))?;
    let text = |range: std::ops::Range<usize>| {
        let field = b.get(range).unwrap_or_default();
        let end = field.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
        field
            .get(..end)
            .unwrap_or_default()
            .iter()
            .map(|&c| {
                if c.is_ascii() {
                    char::from(c)
                } else {
                    '\u{FFFD}'
                }
            })
            .collect::<String>()
    };
    let u32_at = |i: usize| {
        b.get(i..i + 4)
            .and_then(|s| s.try_into().ok())
            .map_or(0, u32::from_le_bytes)
    };
    let i16_at = |i: usize| {
        b.get(i..i + 2)
            .and_then(|s| s.try_into().ok())
            .map_or(0, i16::from_le_bytes)
    };
    let loud = |v: i16| (v != 0x7FFF).then(|| f64::from(v) / 100.0);
    Ok(Some(BextInfo {
        description: text(0..256),
        originator: text(256..288),
        time_reference: u64::from(u32_at(338)) | (u64::from(u32_at(342)) << 32),
        version: b
            .get(346..348)
            .and_then(|s| s.try_into().ok())
            .map_or(0, u16::from_le_bytes),
        loudness_value: loud(i16_at(412)),
        loudness_range: loud(i16_at(414)),
        max_true_peak: loud(i16_at(416)),
    }))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{BextLoudness, bext_chunk, fixed, hundredths};

    #[test]
    fn text_fields_are_ascii_with_replacements_and_nul_padding() {
        assert_eq!(fixed("Čau", 5), b"?au\0\0");
        assert_eq!(fixed("abcdef", 3), b"abc");
        assert_eq!(
            hundredths(Some(-14.005)),
            -1400,
            "-1400.4999… rounds to -1400"
        );
        assert_eq!(hundredths(Some(0.125)), 12, "an exact tie goes to even");
        assert_eq!(hundredths(Some(f64::NEG_INFINITY)), 0x7FFF);
        assert_eq!(hundredths(None), 0x7FFF);
        assert_eq!(hundredths(Some(1e9)), 32767);
    }

    #[test]
    fn the_chunk_has_the_prototypes_layout() {
        // 2026-01-01T12:00:00Z.
        let now = UNIX_EPOCH + Duration::from_hours(490_908);
        let loud = BextLoudness {
            integrated: Some(-14.0),
            range: None,
            true_peak: Some(-1.234),
            momentary_max: Some(-10.0),
            short_term_max: Some(-12.5),
        };
        let chunk = bext_chunk(
            "Title",
            "Sonara",
            5_000_000_000,
            Some(&loud),
            "A=PCM\r\n",
            now,
        );
        assert_eq!(&chunk[..4], b"bext");
        let body = &chunk[8..];
        assert_eq!(body.len(), 602 + 8, "even length after the coding history");
        assert_eq!(&body[320..330], b"2026-01-01");
        assert_eq!(&body[330..338], b"12:00:00");
        assert_eq!(
            u32::from_le_bytes(body[338..342].try_into().unwrap()),
            705_032_704
        );
        assert_eq!(u32::from_le_bytes(body[342..346].try_into().unwrap()), 1);
        assert_eq!(
            i16::from_le_bytes(body[412..414].try_into().unwrap()),
            -1400
        );
        assert_eq!(
            i16::from_le_bytes(body[414..416].try_into().unwrap()),
            0x7FFF
        );
        assert_eq!(i16::from_le_bytes(body[416..418].try_into().unwrap()), -123);
    }
}
