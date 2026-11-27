//! Where Canon and Nikon cameras focused, from the AF blocks of their maker notes, which LibRaw
//! keeps as they are (the library's step 2.17, record 0022; PhotoMatch's plan step 2.81). Their
//! layouts are ExifTool's documentation's (<https://exiftool.org/TagNames/Canon.html#AFInfo2>,
//! <https://exiftool.org/TagNames/Nikon.html>, read on 2026-11-26), none of its code:
//!
//! - **Canon's AFInfo2** (tag 0x0026, or AFInfo3, 0x003c, of the same layout): 16-bit words, the
//!   number of AF areas, how many are valid, the AF image's size, then each area's width, height,
//!   x and y, from the image's middle, "positive Y upwards for EOS models, but apparently downwards
//!   for PowerShot models", and bits for the areas in focus and those selected.
//! - **Nikon's AFInfo2** (tag 0x00b7), by its version, LibRaw's copy starting after the 4 bytes of
//!   the version: where the camera records it, the AF area's middle and size in an AF image's
//!   pixels, from its top left; otherwise the points used, as bits, or the primary point, as a
//!   number, which only a map of the camera's points places. The maps are the Focus Points
//!   plugin's, Apache 2.0, in `af_maps/`.

use std::collections::HashMap;

use crate::af_numbers::{POINTS_105, POINTS_153, POINTS_39, POINTS_51, POINTS_81};

/// A box in shares of the frame as the sensor sees it: its middle, its size where the camera
/// records one, and the frame's width over its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SensorBox {
    pub x: f64,
    pub y: f64,
    pub size: Option<(f64, f64)>,
    pub aspect: f64,
}

/// The box around boxes, each its middle and size, in shares of a frame `aspect` wide over high;
/// `None` for none, or a middle outside the frame.
fn around(boxes: &[(f64, f64, f64, f64)], aspect: f64) -> Option<SensorBox> {
    if boxes.is_empty() || !(aspect > 0.0) {
        return None;
    }
    let left = boxes.iter().map(|b| b.0 - b.2 / 2.0).fold(f64::INFINITY, f64::min);
    let right = boxes.iter().map(|b| b.0 + b.2 / 2.0).fold(f64::NEG_INFINITY, f64::max);
    let top = boxes.iter().map(|b| b.1 - b.3 / 2.0).fold(f64::INFINITY, f64::min);
    let bottom = boxes.iter().map(|b| b.1 + b.3 / 2.0).fold(f64::NEG_INFINITY, f64::max);
    let (x, y) = ((left + right) / 2.0, (top + bottom) / 2.0);
    let (width, height) = (right - left, bottom - top);
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some(SensorBox { x, y, size: (width > 0.0 && height > 0.0).then_some((width.min(1.0), height.min(1.0))), aspect })
}

/// A 16-bit word of a block, in the maker notes' byte order.
fn word(block: &[u8], at: usize, little: bool) -> Option<u16> {
    let bytes: [u8; 2] = block.get(at..at + 2)?.try_into().ok()?;
    Some(if little { u16::from_le_bytes(bytes) } else { u16::from_be_bytes(bytes) })
}

/// Whether bit `k` of the bits starting at byte `at` is set, the lowest of each byte first.
fn bit(block: &[u8], at: usize, k: usize) -> bool {
    block.get(at + k / 8).is_some_and(|byte| byte >> (k % 8) & 1 == 1)
}

/// Where a Canon camera focused, from its AFInfo2 block: the areas in focus, or those selected
/// where none is, a box around them. `model` as LibRaw names it, "EOS R5 Mark II": an EOS's y runs
/// upwards, a PowerShot's down.
pub(crate) fn canon(block: &[u8], little: bool, model: &str) -> Option<SensorBox> {
    let w = |i: usize| word(block, 2 * i, little);
    let count = w(2)? as usize;
    let valid = (w(3)? as usize).min(count);
    let (width, height) = (f64::from(w(6)?), f64::from(w(7)?));
    if valid == 0 || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let flags = count.div_ceil(16);
    let (widths, heights, xs, ys) = (8, 8 + count, 8 + 2 * count, 8 + 3 * count);
    let (in_focus, selected) = (8 + 4 * count, 8 + 4 * count + flags);
    let set = |base: usize, k: usize| w(base + k / 16).is_some_and(|v| v >> (k % 16) & 1 == 1);
    let mut chosen: Vec<usize> = (0..valid).filter(|&k| set(in_focus, k)).collect();
    if chosen.is_empty() {
        chosen = (0..valid).filter(|&k| set(selected, k)).collect();
    }
    let up = model.starts_with("EOS");
    let boxes: Option<Vec<_>> = chosen
        .iter()
        .map(|&k| {
            let (x, y) = (f64::from(w(xs + k)? as i16), f64::from(w(ys + k)? as i16));
            let (aw, ah) = (f64::from(w(widths + k)?), f64::from(w(heights + k)?));
            Some((0.5 + x / width, if up { 0.5 - y / height } else { 0.5 + y / height }, aw / width, ah / height))
        })
        .collect();
    around(&boxes?, width / height)
}

/// The points of a Nikon camera, from its map: each point's middle and size in the camera's own
/// image pixels, the image's size, and the rows in order, each its columns in order.
struct Map {
    width: f64,
    height: f64,
    points: HashMap<String, (f64, f64, f64, f64)>,
    rows: Vec<(char, Vec<u32>)>,
}

/// The maps, by camera, as `key` writes a model; the files as the plugin has them.
const MAPS: &[(&str, &str)] = &[
    ("d300", include_str!("af_maps/nikon/nikon d300.txt")),
    ("d3x", include_str!("af_maps/nikon/nikon d3x.txt")),
    ("d4", include_str!("af_maps/nikon/nikon d4.txt")),
    ("d5", include_str!("af_maps/nikon/nikon d5.txt")),
    ("d500", include_str!("af_maps/nikon/nikon d500.txt")),
    ("d5500", include_str!("af_maps/nikon/nikon d5500.txt")),
    ("d6", include_str!("af_maps/nikon/nikon d6.txt")),
    ("d600", include_str!("af_maps/nikon/nikon d600.txt")),
    ("d700", include_str!("af_maps/nikon/nikon d700.txt")),
    ("d7000", include_str!("af_maps/nikon/nikon d7000.txt")),
    ("d7100", include_str!("af_maps/nikon/nikon d7100.txt")),
    ("d7200", include_str!("af_maps/nikon/nikon d7200.txt")),
    ("d750", include_str!("af_maps/nikon/nikon d750.txt")),
    ("d7500", include_str!("af_maps/nikon/nikon d7500.txt")),
    ("d780-51", include_str!("af_maps/nikon/nikon d780-51.txt")),
    ("d780-81", include_str!("af_maps/nikon/nikon d780-81.txt")),
    ("d800", include_str!("af_maps/nikon/nikon d800.txt")),
    ("d850", include_str!("af_maps/nikon/nikon d850.txt")),
    ("df", include_str!("af_maps/nikon/nikon df.txt")),
    ("z30", include_str!("af_maps/nikon/nikon z 30.txt")),
    ("z5", include_str!("af_maps/nikon/nikon z 5.txt")),
    ("z50", include_str!("af_maps/nikon/nikon z 50.txt")),
    ("z6", include_str!("af_maps/nikon/nikon z 6.txt")),
    ("z7", include_str!("af_maps/nikon/nikon z 7.txt")),
    ("z9", include_str!("af_maps/nikon/nikon z 9.txt")),
    ("zf", include_str!("af_maps/nikon/nikon z f.txt")),
    ("z50_2", include_str!("af_maps/nikon/nikon z50_2.txt")),
    ("z5_2", include_str!("af_maps/nikon/nikon z5_2.txt")),
    ("z6_3", include_str!("af_maps/nikon/nikon z6_3.txt")),
];

/// The cameras that share another's map, as the plugin's `NikonDuplicates.lua` has them.
const SAME_MAP: &[(&str, &str)] = &[
    ("d3", "d700"),
    ("d3s", "d700"),
    ("d4s", "d4"),
    ("d300s", "d300"),
    ("d610", "d600"),
    ("d800e", "d800"),
    ("d810", "d800"),
    ("d5600", "d5500"),
    ("d5300", "d5500"),
    ("d5200", "d5500"),
    ("z6_2", "z6"),
    ("z7_2", "z7"),
    ("z8", "z9"),
    ("zfc", "z30"),
];

/// A model as the maps are keyed: lower case, without spaces, as "Z 6_2" is "z6_2".
fn key(model: &str) -> String {
    model.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect()
}

/// The map of a camera, or of the camera whose map it shares.
fn map_for(model: &str) -> Option<Map> {
    let key = key(model);
    let key = SAME_MAP.iter().find(|(from, _)| *from == key).map_or(key.as_str(), |(_, to)| to).to_string();
    MAPS.iter().find(|(name, _)| *name == key).and_then(|(_, text)| parse(text))
}

/// A map's text: the image's size from a comment, `8256 x 5504` or `8256 × 5504`; the points'
/// common size, `focusPointDimens = {w, h}`; each point, `A1 = {x, y}` or `{x, y, w, h}`.
fn parse(text: &str) -> Option<Map> {
    let numbers = |s: &str| -> Vec<f64> { s.split(|c: char| !c.is_ascii_digit()).filter(|n| !n.is_empty()).filter_map(|n| n.parse().ok()).collect() };
    let mut frame = None;
    // The points' common size, wherever in the file it's given.
    let common = text.lines().map(str::trim).find_map(|line| {
        let (name, value) = line.split_once('=')?;
        match (name.trim(), &numbers(value)[..]) {
            ("focusPointDimens", &[w, h]) => Some((w, h)),
            _ => None,
        }
    });
    let mut points = HashMap::new();
    let mut rows: Vec<(char, Vec<u32>)> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(comment) = line.strip_prefix("--") {
            if frame.is_none() && (comment.contains(" x ") || comment.contains('×')) {
                if let [w, h, ..] = numbers(comment)[..] {
                    frame = Some((w, h));
                }
            }
            continue;
        }
        let Some((name, value)) = line.split_once('=') else { continue };
        let (name, value) = (name.trim(), numbers(value));
        let mut chars = name.chars();
        let (Some(row), Ok(column)) = (chars.next(), chars.as_str().parse::<u32>()) else { continue };
        if !row.is_ascii_uppercase() {
            continue;
        }
        let point = match value[..] {
            [x, y, w, h] => (x, y, w, h),
            [x, y] => {
                let (w, h) = common.unwrap_or((0.0, 0.0));
                (x, y, w, h)
            }
            _ => continue,
        };
        points.insert(name.to_string(), point);
        match rows.iter_mut().find(|(r, _)| *r == row) {
            Some((_, columns)) => columns.push(column),
            None => rows.push((row, vec![column])),
        }
    }
    rows.sort_by_key(|(row, _)| *row);
    for (_, columns) in &mut rows {
        columns.sort_unstable();
    }
    let (width, height) = frame?;
    (width > 0.0 && height > 0.0 && !points.is_empty()).then_some(Map { width, height, points, rows })
}

impl Map {
    /// A point's middle and size in the camera's image pixels: from the map, or, for a point
    /// between those it lists, as the 153-point cameras' maps list only the 55 that can be chosen,
    /// its row's height and its column's place, each between the nearest the map has.
    fn point(&self, name: &str) -> Option<(f64, f64, f64, f64)> {
        if let Some(point) = self.points.get(name) {
            return Some(*point);
        }
        let row = name.chars().next()?;
        let column: u32 = name[1..].parse().ok()?;
        let mean = |values: Vec<f64>| (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);
        let at_row = |r: char| mean(self.points.iter().filter(|(n, _)| n.starts_with(r)).map(|(_, p)| p.1).collect());
        let at_column = |c: u32| mean(self.points.iter().filter(|(n, _)| n[1..].parse() == Ok(c)).map(|(_, p)| p.0).collect());
        let between = |at: &dyn Fn(i64) -> Option<f64>, k: i64| -> Option<f64> {
            if let Some(v) = at(k) {
                return Some(v);
            }
            let below = (1..=k).find_map(|d| at(k - d).map(|v| (d, v)));
            let above = (1..=40).find_map(|d| at(k + d).map(|v| (d, v)));
            let ((db, vb), (da, va)) = (below?, above?);
            Some(vb + (va - vb) * db as f64 / (db + da) as f64)
        };
        let y = between(&|k| u8::try_from(k).ok().map(char::from).and_then(at_row), row as i64)?;
        let x = between(&|k| u32::try_from(k).ok().and_then(at_column), i64::from(column))?;
        let mut sizes: Vec<(f64, f64)> = self.points.values().map(|p| (p.2, p.3)).collect();
        sizes.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (w, h) = sizes[sizes.len() / 2];
        Some((x, y, w, h))
    }

    /// The box around the named points, in shares of the image.
    fn around(&self, names: &[&str]) -> Option<SensorBox> {
        let boxes: Option<Vec<_>> = names
            .iter()
            .map(|name| self.point(name).map(|(x, y, w, h)| (x / self.width, y / self.height, w / self.width, h / self.height)))
            .collect();
        around(&boxes?, self.width / self.height)
    }

    /// The points as the bits of AFPointsUsed run, each row's in turn, as A1 to A9, then B1 to B11.
    fn in_order(&self) -> Vec<String> {
        self.rows.iter().flat_map(|(row, columns)| columns.iter().map(move |c| format!("{row}{c}"))).collect()
    }
}

/// The AF area's box where the camera records it in an AF image's pixels: its width and height,
/// then the area's middle, x and y, and its width and height, from byte `at`.
fn area(block: &[u8], at: usize, little: bool) -> Option<SensorBox> {
    let w = |k: usize| word(block, at + 2 * k, little).map(f64::from);
    let (width, height, x, y, aw, ah) = (w(0)?, w(1)?, w(2)?, w(3)?, w(4)?, w(5)?);
    if width <= 0.0 || height <= 0.0 || x > width || y > height || (x, y) == (0.0, 0.0) {
        return None;
    }
    Some(SensorBox { x: x / width, y: y / height, size: (aw > 0.0 && ah > 0.0).then_some((aw / width, ah / height)), aspect: width / height })
}

/// Where a Nikon camera focused, from its AFInfo2 block of `version`, as LibRaw has it, `0101` as
/// 101: the AF area where the camera records it, else the points it used or its primary point, on
/// its map. `model` as LibRaw names it, "Z 6_2". `None` where it records neither, or no map has
/// the camera, as the D780's two, chosen by a field this doesn't read, and the 11-point cameras'.
pub(crate) fn nikon(block: &[u8], version: u32, little: bool, model: &str) -> Option<SensorBox> {
    let byte = |at: usize| block.get(at).copied();
    // The primary point, numbered from the middle out, on a map.
    let primary = |numbers: &[&str], at: usize| -> Option<SensorBox> {
        let n = usize::from(byte(at)?);
        let name = *numbers.get(n.checked_sub(1)?)?;
        map_for(model)?.around(&[name])
    };
    // The points used, as bits from byte 6, each row of the map's in turn.
    let used = || -> Option<SensorBox> {
        let map = map_for(model)?;
        let names = map.in_order();
        let chosen: Vec<&str> = names.iter().enumerate().filter(|(k, _)| bit(block, 6, *k)).map(|(_, n)| n.as_str()).collect();
        map.around(&chosen)
    };
    match version {
        // Cameras of 2007 to 2016: contrast-detect AF records its area; phase detect its primary point.
        100 => match (byte(0)?, byte(2)?) {
            (1, _) => area(block, 12, little),
            (_, 1) => primary(&POINTS_51, 3),
            (_, 3) => primary(&POINTS_39, 3),
            _ => None,
        },
        // D5, D500, D850, D7500, D3400 and D3500.
        101 => match (byte(0)?, byte(2)?) {
            (1, _) => area(block, 66, little),
            (_, 1) => primary(&POINTS_51, 64),
            (_, 7) => primary(&POINTS_153, 64),
            _ => None,
        },
        // Expeed 6 and 7: the area where the camera says it's recorded, else the points used.
        300..=399 => match byte(3)? {
            1 => area(block, 38, little),
            _ => used(),
        },
        400..=499 => match byte(3)? {
            1 => area(block, 58, little),
            _ => used(),
        },
        _ => None,
    }
    .or_else(|| {
        // Expeed 6 where no bit was set: its primary point, numbered as the D6's 105 points or the
        // 81 auto-area points of the mirrorless bodies.
        if !(300..=399).contains(&version) {
            return None;
        }
        primary(if key(model) == "d6" { &POINTS_105 } else { &POINTS_81 }, 52)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_reads_with_its_middle_point_in_the_middle() {
        for (name, text) in MAPS {
            let map = parse(text).unwrap_or_else(|| panic!("{name} doesn't read"));
            let middle = match map.points.len() {
                55 => "E9",
                51 | 39 => "C6",
                81 => "E5",
                105 => "D8",
                405 => "H14",
                299 => "G12",
                231 => "F11",
                n => panic!("{name}: {n} points"),
            };
            let (x, y, ..) = map.point(middle).unwrap();
            assert!((x / map.width - 0.5).abs() < 0.03 && (y / map.height - 0.5).abs() < 0.03, "{name}: {middle} at {x}, {y} of {} × {}", map.width, map.height);
        }
    }

    #[test]
    fn a_point_between_those_a_map_lists_lies_between_them() {
        let map = map_for("D850").unwrap();
        assert!(!map.points.contains_key("D10"));
        let (x, y, ..) = map.point("D10").unwrap();
        let (c9, e11) = (map.point("C9").unwrap(), map.point("E11").unwrap());
        assert!(x > c9.0 && x < e11.0 && y > c9.1 && y < e11.1, "D10 at {x}, {y}");
    }

    /// A Canon block of one area, as the R5 Mark II's: 1053 areas, one valid, selected and in focus.
    fn canon_block(x: i16, y: i16, in_focus: bool) -> Vec<u8> {
        let count = 1053usize;
        let mut words = vec![0u16; 8 + 4 * count + 2 * count.div_ceil(16) + 1];
        words[..8].copy_from_slice(&[0, 9, count as u16, 1, 8192, 5464, 8192, 5464]);
        words[8] = 288;
        words[8 + count] = 288;
        words[8 + 2 * count] = x as u16;
        words[8 + 3 * count] = y as u16;
        if in_focus {
            words[8 + 4 * count] = 1;
        }
        words[8 + 4 * count + count.div_ceil(16)] = 1;
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn canon_s_area_runs_up_from_the_middle_on_an_eos_and_down_on_a_powershot() {
        let eos = canon(&canon_block(-84, 344, true), true, "EOS R5 Mark II").unwrap();
        assert!((eos.x - (0.5 - 84.0 / 8192.0)).abs() < 1e-9 && (eos.y - (0.5 - 344.0 / 5464.0)).abs() < 1e-9, "{eos:?}");
        assert_eq!(eos.size, Some((288.0 / 8192.0, 288.0 / 5464.0)));
        let powershot = canon(&canon_block(-84, 344, true), true, "PowerShot G7 X").unwrap();
        assert!((powershot.y - (0.5 + 344.0 / 5464.0)).abs() < 1e-9);
        // Selected but not in focus, as the R6 Mark III's: still where the camera focused.
        assert!(canon(&canon_block(1508, 406, false), true, "EOS R6 Mark III").is_some());
    }

    #[test]
    fn nikon_s_recorded_area_and_its_points_used_and_primary_point_are_placed() {
        // Expeed 6, as the Z 6II's: the area recorded, from the top left.
        let mut z6 = vec![0u8; 52];
        z6[..4].copy_from_slice(&[2, 0xc1, 0, 1]);
        for (k, v) in [6048u16, 4024, 3024, 1006, 285, 308].iter().enumerate() {
            z6[38 + 2 * k..40 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
        let found = nikon(&z6, 301, true, "Z 6_2").unwrap();
        assert!((found.x - 0.5).abs() < 1e-9 && (found.y - 0.25).abs() < 1e-9, "{found:?}");
        // Expeed 7 in auto-area, as the Z 8's: the points used on the Z 9's map, H14 alone.
        let mut z8 = vec![0u8; 76];
        z8[..4].copy_from_slice(&[2, 0xc5, 0x0a, 0]);
        let h14 = 7 * 27 + 13;
        z8[6 + h14 / 8] |= 1 << (h14 % 8);
        let found = nikon(&z8, 400, true, "Z 8").unwrap();
        assert!((found.x - 0.5).abs() < 0.01 && (found.y - 0.5).abs() < 0.01, "{found:?}");
        // A D850 through its viewfinder: primary point 1, E9, the middle.
        let mut d850 = vec![0u8; 80];
        d850[2] = 7;
        d850[64] = 1;
        let found = nikon(&d850, 101, true, "D850").unwrap();
        assert!((found.x - 0.5).abs() < 0.01 && (found.y - 0.5).abs() < 0.01, "{found:?}");
        // A camera no map has.
        assert_eq!(nikon(&d850, 101, true, "D3500"), None);
    }
}
