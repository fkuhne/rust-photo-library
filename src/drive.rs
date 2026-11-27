//! How the camera's shutter was released, one frame at a time or continuously, from what each maker
//! records, as ExifTool's tag pages give the values (read on 2026-11-27), and the fraction of a
//! second EXIF adds to the time a photo was taken: for an app to find a burst, frames of one
//! camera shot a fraction of a second apart or in its continuous drive mode (record 0023).

use serde::{Deserialize, Serialize};

/// How the shutter was released, where the file says so in a way the library reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Drive {
    /// One frame each time the shutter is pressed.
    Single,
    /// Frames for as long as the shutter is held: a burst.
    Continuous,
}

/// Sony's ReleaseMode2, from its 0x9400 block: 1 Continuous, 2 Continuous - Exposure Bracketing,
/// 5 Continuous - Burst, 17 Continuous - Burst 2, 19 Continuous - Speed/Advance Priority, 26
/// Continuous Low, 27 Continuous - High Sensitivity, 29 Continuous - Tele-zoom Advance Priority; 0
/// Normal, 3 DRO or White Balance Bracketing, 6 and 146 a frame during a movie, 18 Normal - iAuto+,
/// 23 Single-frame - Exposure Bracketing, 28 Smile Shutter. The others, as panoramas, HDR and Multi
/// Frame NR, the camera combines into one photo: not said.
pub fn sony(release_mode2: u8) -> Option<Drive> {
    match release_mode2 {
        1 | 2 | 5 | 17 | 19 | 26 | 27 | 29 => Some(Drive::Continuous),
        0 | 3 | 6 | 18 | 23 | 28 | 146 => Some(Drive::Single),
        _ => None,
    }
}

/// Canon's ContinuousDrive, from its CameraSettings: 0 Single, 6 Silent Single, 9 Single, Silent;
/// 1 Continuous, 3 Continuous, Speed Priority, 4 Continuous, Low, 5 Continuous, High, 8 Continuous,
/// High+, 10 Continuous, Silent; 2 Movie, not said.
pub fn canon(continuous_drive: i16) -> Option<Drive> {
    match continuous_drive {
        0 | 6 | 9 => Some(Drive::Single),
        1 | 3 | 4 | 5 | 8 | 10 => Some(Drive::Continuous),
        _ => None,
    }
}

/// Nikon's ShootingMode, tag 0x0089: its bit 0, Continuous.
pub fn nikon(shooting_mode: u16) -> Option<Drive> {
    Some(if shooting_mode & 1 != 0 { Drive::Continuous } else { Drive::Single })
}

/// Fujifilm's DriveMode, from its DriveSettings: 0 Single, 1 Continuous Low, 2 Continuous High; -1,
/// LibRaw's where it found none, not said.
pub fn fujifilm(drive_mode: i16) -> Option<Drive> {
    match drive_mode {
        0 => Some(Drive::Single),
        1 | 2 => Some(Drive::Continuous),
        _ => None,
    }
}

/// Pentax's and Ricoh's DriveMode, tag 0x0034, its first byte: 0 Single-frame; 1 Continuous, 2
/// Continuous (Lo), 3 Burst, 4 Continuous (Medium), 5 Continuous (Low); 255 Video, not said.
pub fn pentax(drive_mode: u8) -> Option<Drive> {
    match drive_mode {
        0 => Some(Drive::Single),
        1..=5 => Some(Drive::Continuous),
        _ => None,
    }
}

/// EXIF's SubSecTimeOriginal, the digits after the second's point, as milliseconds: "83" is 830,
/// "09" 90, "669261" 669. `None` for anything but digits.
pub fn milliseconds(subsec: &str) -> Option<u16> {
    let digits = subsec.trim().trim_end_matches('\0').trim();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let first: String = digits.chars().chain(std::iter::repeat('0')).take(3).collect();
    first.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_maker_s_values_say_single_or_continuous_and_the_others_nothing() {
        // The samples' own: the A6400's 26 and the A7S III's 1, the others' 0 (2026-11-27).
        assert_eq!((sony(26), sony(1), sony(0), sony(7)), (Some(Drive::Continuous), Some(Drive::Continuous), Some(Drive::Single), None));
        assert_eq!((canon(0), canon(5), canon(2)), (Some(Drive::Single), Some(Drive::Continuous), None));
        assert_eq!((nikon(0), nikon(0x11)), (Some(Drive::Single), Some(Drive::Continuous)));
        assert_eq!((fujifilm(-1), fujifilm(0), fujifilm(2)), (None, Some(Drive::Single), Some(Drive::Continuous)));
        // The K-3 Mark III sample's 1.
        assert_eq!((pentax(1), pentax(0), pentax(255)), (Some(Drive::Continuous), Some(Drive::Single), None));
    }

    #[test]
    fn a_fraction_of_a_second_as_written_is_its_milliseconds() {
        assert_eq!(milliseconds("83"), Some(830));
        assert_eq!(milliseconds("09"), Some(90));
        assert_eq!(milliseconds("943"), Some(943));
        assert_eq!(milliseconds("669261"), Some(669));
        assert_eq!(milliseconds(" 7 "), Some(700));
        assert_eq!((milliseconds(""), milliseconds("1a")), (None, None));
    }
}
