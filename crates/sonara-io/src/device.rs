//! Audio devices as an enumeration lists them, and their ids.
//!
//! The settings save a device's id as text (`settings.audio.output_device` and
//! `input_device`), so its form is a contract (CTR-01): the raw bytes of miniaudio's
//! `ma_device_id`, with the trailing zero bytes trimmed, as lowercase hex
//! (`id_to_hex`/`hex_to_id` in `native/src/engine.cpp`).

use std::fmt;
use std::str::FromStr;

use crate::ffi::DEVICE_ID_BYTES;

/// The id of an audio device: the raw bytes of miniaudio's `ma_device_id`.
///
/// It displays as the prototype's saved form: lowercase hex of the bytes up to the last non-zero
/// one. On Core Audio that is the device's UID string, on the null backend it is empty.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DeviceId {
    raw: [u8; DEVICE_ID_BYTES],
}

impl DeviceId {
    pub(crate) const fn from_raw(raw: [u8; DEVICE_ID_BYTES]) -> Self {
        Self { raw }
    }

    pub(crate) const fn raw(&self) -> &[u8; DEVICE_ID_BYTES] {
        &self.raw
    }

    /// The raw bytes without the trailing zero bytes.
    fn trimmed(&self) -> &[u8] {
        let len = self
            .raw
            .iter()
            .rposition(|byte| *byte != 0)
            .map_or(0, |last| last + 1);
        self.raw.get(..len).unwrap_or(&[])
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.trimmed()
            .iter()
            .try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId(\"{self}\")")
    }
}

/// Why a text is not a device id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid device id")]
pub struct InvalidDeviceId;

impl FromStr for DeviceId {
    type Err = InvalidDeviceId;

    /// Parses the saved form with the prototype's rules: lowercase hex digits, an even number of
    /// them, at most 256 bytes. The missing bytes are zero, so `""` is the all-zero id.
    fn from_str(hex: &str) -> Result<Self, Self::Err> {
        let digits = hex.as_bytes();
        if !digits.len().is_multiple_of(2) || digits.len() / 2 > DEVICE_ID_BYTES {
            return Err(InvalidDeviceId);
        }
        let mut raw = [0; DEVICE_ID_BYTES];
        // The length is even, so the pairs cover every digit.
        let (pairs, _) = digits.as_chunks::<2>();
        for (byte, &[high, low]) in raw.iter_mut().zip(pairs) {
            *byte = (hex_digit(high)? << 4) | hex_digit(low)?;
        }
        Ok(Self { raw })
    }
}

fn hex_digit(digit: u8) -> Result<u8, InvalidDeviceId> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        _ => Err(InvalidDeviceId),
    }
}

/// Whether a device plays or captures audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// An output device.
    Playback,
    /// An input device.
    Capture,
}

impl DeviceKind {
    /// The prototype's name of the kind: `"playback"` or `"capture"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Playback => "playback",
            Self::Capture => "capture",
        }
    }
}

/// A device as [`Engine::devices`](crate::Engine::devices) lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The device's id, as the settings save it.
    pub id: DeviceId,
    /// The device's name.
    pub name: String,
    /// Whether it plays or captures.
    pub kind: DeviceKind,
    /// Whether it is the system default of its kind.
    pub is_default: bool,
    /// The most channels among its native formats, or 2 when it lists none (the prototype's
    /// `channels_of`).
    pub channels: u32,
}

impl DeviceInfo {
    /// A device from the enumeration; `native_channels` is 0 when the device lists no native
    /// format.
    pub(crate) fn new(
        id: DeviceId,
        name: String,
        kind: DeviceKind,
        is_default: bool,
        native_channels: u32,
    ) -> Self {
        let channels = if native_channels == 0 {
            2
        } else {
            native_channels
        };
        Self {
            id,
            name,
            kind,
            is_default,
            channels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEVICE_ID_BYTES, DeviceId, DeviceInfo, DeviceKind, InvalidDeviceId};

    fn raw_from(bytes: &[u8]) -> [u8; DEVICE_ID_BYTES] {
        let mut raw = [0; DEVICE_ID_BYTES];
        raw[..bytes.len()].copy_from_slice(bytes);
        raw
    }

    /// The prototype's id of the owner's built-in speakers (`sonara devices` on the macOS host):
    /// the Core Audio UID `BuiltInSpeakerDevice`.
    #[test]
    fn displays_the_prototype_hex_of_a_core_audio_uid() {
        let id = DeviceId::from_raw(raw_from(b"BuiltInSpeakerDevice"));
        assert_eq!(id.to_string(), "4275696c74496e537065616b6572446576696365");
        assert_eq!(
            "4275696c74496e537065616b6572446576696365".parse::<DeviceId>(),
            Ok(id)
        );
    }

    #[test]
    fn trims_trailing_zero_bytes_only() {
        let id = DeviceId::from_raw(raw_from(&[0, 0xff, 0, 0x0a]));
        assert_eq!(id.to_string(), "00ff000a");
        let id = DeviceId::from_raw(raw_from(&[0, 0xff]));
        assert_eq!(id.to_string(), "00ff");
        // The null backend's devices: an all-zero id.
        assert_eq!(DeviceId::from_raw([0; DEVICE_ID_BYTES]).to_string(), "");
    }

    #[test]
    fn parses_every_byte_value_and_round_trips() {
        let bytes: Vec<u8> = (0..=255).rev().collect();
        let id = DeviceId::from_raw(raw_from(&bytes));
        let hex = id.to_string();
        assert_eq!(hex.len(), 2 * 255, "the trailing zero byte is trimmed");
        assert_eq!(hex.parse::<DeviceId>(), Ok(id));
        // 256 bytes fit exactly.
        assert!("ff".repeat(DEVICE_ID_BYTES).parse::<DeviceId>().is_ok());
        // Leading and inner zeros are kept; a parsed id equals the enumerated one.
        assert_eq!(
            "00ff00".parse::<DeviceId>(),
            Ok(DeviceId::from_raw(raw_from(&[0, 0xff])))
        );
        assert_eq!(
            "".parse::<DeviceId>().map(|id| id.to_string()).as_deref(),
            Ok("")
        );
    }

    /// The prototype's `hex_to_id` rejects these (its `start` answers "invalid output device
    /// id"; checked on the prototype on 2026-10-22).
    #[test]
    fn rejects_what_the_prototype_rejects() {
        for text in ["zz", "abc", "AB", "0x12", " 12", "12 ", "é1", "\u{0}0"] {
            assert_eq!(text.parse::<DeviceId>(), Err(InvalidDeviceId), "{text:?}");
        }
        assert_eq!(
            "00".repeat(DEVICE_ID_BYTES + 1).parse::<DeviceId>(),
            Err(InvalidDeviceId)
        );
    }

    #[test]
    fn falls_back_to_two_channels_without_native_formats() {
        let id = DeviceId::from_raw([0; DEVICE_ID_BYTES]);
        let info = DeviceInfo::new(id.clone(), "a".into(), DeviceKind::Playback, true, 0);
        assert_eq!(info.channels, 2);
        let info = DeviceInfo::new(id, "a".into(), DeviceKind::Capture, false, 6);
        assert_eq!(info.channels, 6);
        assert_eq!(DeviceKind::Playback.as_str(), "playback");
        assert_eq!(DeviceKind::Capture.as_str(), "capture");
    }
}
