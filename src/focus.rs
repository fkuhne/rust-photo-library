//! Where a photo is sharp, as a camera's focus peaking marks it: from a RAW file's own sensor
//! data, never its preview, whose sharpening would make a soft photo look crisp; from an image
//! file's pixels, which are all it has (the library's plan step 2.13, record 0018; PhotoMatch's
//! plan step 2.75).
//!
//! A brightness for each full colour of the sensor, its square root so that the sensor's noise,
//! which grows with the light, is about the same everywhere; then, across and down, the slope
//! over one step either side against the slope over three: a sharp edge is steeper close up than
//! further out, a soft one isn't. A step is sharp where the close slope is steeper by a share, so
//! that a soft edge of high contrast doesn't count, and steeper by more than the photo's own noise
//! times the level's factor, so that its noise doesn't; and where the step beside it along the
//! edge is too, so that a hot photosite or a grain of noise doesn't. And as focus, not sharpness,
//! is the question (the library's step 2.14, record 0019): sharp steps stand only where, around
//! them, sharp detail runs both across and down, as out-of-focus blur spreads every way and a blur
//! of motion leaves crisp streaks one way only. The marks are made at the size of the image they go over,
//! upright, each pixel sharp where any step of the sensor under it is.

use std::path::Path;

use image::{DynamicImage, GrayImage, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::clipping::{black_of, colours_of, sensor_for, Sensor};
use crate::photo::Orientation;
use crate::problem::Problem;
use crate::raw;

/// How readily a step counts as sharp, as a camera's Peaking Level: High marks the most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    Low,
    Mid,
    High,
}

impl Level {
    /// How many times the photo's noise the close slope must pass the far one by. Noise alone,
    /// its close slope spread 0.71 times the noise, passes 3 times it once in 45,000 steps, and
    /// two such steps beside each other along an edge next to never (computed).
    pub fn factor(self) -> f32 {
        match self {
            Level::Low => 7.0,
            Level::Mid => 4.5,
            Level::High => 3.0,
        }
    }

    /// How many times the far slope the close one must be. A sharp step is 3 times; blurred by
    /// 1 step, 2.05; by 1.5, 1.55; by 2.5, 1.21, whatever its contrast (computed, for a Gaussian blur).
    pub fn steepness(self) -> f32 {
        match self {
            Level::Low => 2.2,
            Level::Mid => 1.8,
            Level::High => 1.6,
        }
    }

    /// Its name, as kept in a mark's file name.
    pub fn name(self) -> &'static str {
        match self {
            Level::Low => "low",
            Level::Mid => "mid",
            Level::High => "high",
        }
    }
}

/// Where the camera focused, as it records it (the library's step 2.16, record 0021): a box in
/// fractions of the upright image, its middle and its size.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Whether the photo is in focus inside the camera's focus point's box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// Focus marks inside: in focus and still there.
    InFocus,
    /// Detail over the noise inside, none of it in focus.
    NotInFocus,
    /// Too little detail inside to tell, a plain wall or sky.
    CantTell,
}

/// The box's side, as a share of the image's short side: no file read here gives its AF area's size.
const POINT_SIDE: f64 = 0.06;
/// In focus where at least this share of the box's pixels is marked.
const IN_FOCUS_SHARE: f64 = 0.01;
/// Not in focus where fewer are, but at least this share of its steps is soft detail, over the
/// noise but no steeper close up than further out…
const SOFT_SHARE: f64 = 0.03;
/// …and steep steps, sharp but unmarked, as a lone straight edge or one in near-black noise, are
/// fewer than this share of the soft ones: a box with those can't tell.
const STEEP_OF_SOFT: f64 = 0.25;

/// Where a RAW file's camera focused, from its maker notes as LibRaw reads them: Sony's
/// `FocusLocation`, its frame's width and height and the point's x and y; Fujifilm's `FocusPixel`,
/// over its embedded JPEG's size, as the Exif image size gives it for a RAF file; Olympus's
/// `AFPointSelected`, a box in shares of the frame. Turned as the photo is. `None` where none is
/// recorded, zeros or a point outside its frame; and for every other maker, until their blocks are
/// read (PhotoMatch's step 2.81).
pub fn point(path: &Path) -> Result<Option<Point>, Problem> {
    let mut image = raw::open(path)?;
    let ptr: *mut rsraw_sys::libraw_data_t = image.as_mut();
    // SAFETY: LibRaw's own data, opened, read only here.
    let data: &rsraw_sys::libraw_data_t = unsafe { &*ptr };
    let make: String = data.idata.make.iter().take_while(|c| **c != 0).map(|c| *c as u8 as char).collect();
    let notes = &data.makernotes;
    // The point in shares of the frame as the sensor sees it, and that frame's width over height.
    let sensor = if make.starts_with("Sony") {
        let [w, h, x, y] = notes.sony.FocusLocation.map(f64::from);
        (w > 0.0 && h > 0.0 && (x, y) != (0.0, 0.0) && x <= w && y <= h).then_some((x / w, y / h, w / h))
    } else if make.starts_with("Fujifilm") || make.starts_with("FUJIFILM") {
        let [x, y] = notes.fuji.FocusPixel.map(f64::from);
        let (tw, th) = (data.thumbnail.twidth as f64, data.thumbnail.theight as f64);
        let (w, h) = (tw.max(th), tw.min(th));
        (w > 0.0 && h > 0.0 && (x, y) != (0.0, 0.0) && x <= w && y <= h).then_some((x / w, y / h, w / h))
    } else if make.starts_with("Olympus") || make.starts_with("OM ") || make.starts_with("OM Digital") {
        let [_, x1, y1, x2, y2] = notes.olympus.AFPointSelected;
        let (x, y) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        let (w, h) = (data.sizes.width as f64, data.sizes.height as f64);
        ((x, y) != (0.0, 0.0) && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) && h > 0.0).then_some((x, y, w / h))
    } else {
        None
    };
    Ok(sensor.map(|(x, y, aspect)| upright_point(x, y, aspect, data.sizes.flip)))
}

/// A point at `x`, `y` of a frame `aspect` wide over high, as the sensor sees it, as a box in the
/// upright image, turned as LibRaw's `flip` says: 3 half round, 5 a quarter left, 6 a quarter right.
fn upright_point(x: f64, y: f64, aspect: f64, flip: i32) -> Point {
    let ((ux, uy), aspect) = match flip {
        3 => ((1.0 - x, 1.0 - y), aspect),
        5 => ((y, 1.0 - x), 1.0 / aspect),
        6 => ((1.0 - y, x), 1.0 / aspect),
        _ => ((x, y), aspect),
    };
    // A square, its side a share of the short side.
    let (width, height) = if aspect >= 1.0 { (POINT_SIDE / aspect, POINT_SIDE) } else { (POINT_SIDE, POINT_SIDE * aspect) };
    Point { x: ux, y: uy, width, height }
}

/// The box as the sensor sees it, from its upright fractions: the turn undone.
fn sensor_box(point: &Point, flip: i32) -> (f64, f64, f64, f64) {
    let (x, y, w, h) = (point.x, point.y, point.width, point.height);
    match flip {
        3 => (1.0 - x, 1.0 - y, w, h),
        5 => (1.0 - y, x, h, w),
        6 => (y, 1.0 - x, h, w),
        _ => (x, y, w, h),
    }
}

/// Whether a RAW file is in focus inside the box `point`, at `level`, as its focus marks for an
/// image shown at `shown` say.
pub fn judge_raw(path: &Path, shown: (u32, u32), level: Level, point: &Point) -> Result<Verdict, Problem> {
    let mut image = raw::open(path)?;
    image.unpack().map_err(|e| Problem::DevelopFailed { cause: format!("LibRaw can't read the RAW data of {}: {e}", path.display()) })?;
    let ptr: *mut rsraw_sys::libraw_data_t = image.as_mut();
    let colours = colours_of(ptr);
    // SAFETY: as in `of_raw`.
    let data: &rsraw_sys::libraw_data_t = unsafe { &*ptr };
    let Sensor { area, out, .. } = sensor_for(data, shown)?;
    let plane = plane_of_raw(data, &colours, area).ok_or_else(|| Problem::DevelopFailed { cause: format!("{} has no RAW data PhotoMatch can read for focus", path.display()) })?;
    Ok(verdict(&plane, (out.0.max(1) as usize, out.1.max(1) as usize), level, sensor_box(point, data.sizes.flip)))
}

/// Whether the plane is in focus inside the box, its middle and size in shares of the frame: in
/// focus where enough of its pixels are marked, at an image of `out`; not in focus only where its
/// detail is plainly soft, over the noise but no steeper close up than further out, and few of its
/// steps are steep; else it can't tell, as for a plain part, a lone straight edge the crossing rule
/// leaves unmarked, or detail lost in near-black noise.
fn verdict(plane: &Plane, out: (usize, usize), level: Level, (x, y, w, h): (f64, f64, f64, f64)) -> Verdict {
    let marks = marks_at(plane, out, level);
    let span = |middle: f64, size: f64, n: usize| {
        let from = (((middle - size / 2.0) * n as f64).floor().max(0.0) as usize).min(n.saturating_sub(1));
        let to = (((middle + size / 2.0) * n as f64).ceil() as usize).clamp(from + 1, n);
        from..to
    };
    let (xs, ys) = (span(x, w, out.0), span(y, h, out.1));
    let inside = xs.len() * ys.len();
    let marked = ys.clone().flat_map(|row| xs.clone().map(move |col| row * out.0 + col)).filter(|i| marks.sharp[*i]).count();
    if inside > 0 && marked as f64 >= IN_FOCUS_SHARE * inside as f64 {
        return Verdict::InFocus;
    }
    // Each step's detail over the noise of its brightness, across and down: steep, as a sharp
    // edge's, or soft, no steeper close up than three steps out.
    let (pw, ph) = (plane.width, plane.height);
    let (noise, _) = noise_of(plane);
    let thresholds = noise.map(|n| (n * level.factor()).max(plane.least));
    let steepness = level.steepness();
    let v = &plane.values;
    let (pxs, pys) = (span(x, w, pw), span(y, h, ph));
    let (mut soft, mut steep, mut steps) = (0usize, 0usize, 0usize);
    for py in pys.filter(|py| (3..ph.saturating_sub(3)).contains(py)) {
        for px in pxs.clone().filter(|px| (3..pw.saturating_sub(3)).contains(px)) {
            let i = py * pw + px;
            steps += 1;
            let threshold = thresholds[band_of(plane, v[i])];
            let look = |a: f32, b: f32, far_a: f32, far_b: f32| {
                let (close, far) = ((b - a).abs() / 2.0, (far_b - far_a).abs() / 6.0);
                (close > threshold, close - far > threshold && close > steepness * far)
            };
            let (across, down) = (look(v[i - 1], v[i + 1], v[i - 3], v[i + 3]), look(v[i - pw], v[i + pw], v[i - 3 * pw], v[i + 3 * pw]));
            if across.1 || down.1 {
                steep += 1;
            } else if across.0 || down.0 {
                soft += 1;
            }
        }
    }
    let plainly_soft = steps > 0 && soft as f64 >= SOFT_SHARE * steps as f64 && (steep as f64) < STEEP_OF_SOFT * soft as f64;
    if plainly_soft { Verdict::NotInFocus } else { Verdict::CantTell }
}

/// The median size of a normal value, as a share of its spread: Φ⁻¹(0.75).
const MEDIAN_OF_SIZE: f32 = 0.67449;

/// The smallest detail that counts, as a share of the brightness's range, for a photo so clean
/// that its noise is next to nothing.
const LEAST_DETAIL: f32 = 0.004;

/// An image file's smallest detail, in its 255 levels: compression flattens its noise, which then
/// can't tell its own edges, 8 by 8 pixels, from a photo's.
const LEAST_IN_IMAGE: f32 = 3.0;

/// Sharpness is judged around each step: a step's mark stands only where, within `REACH` cells of
/// `CELL` steps every way of its own, about 96 steps a side, sharp steps run both across and down,
/// the fewer at least `CROSSING` of the more. A blur of motion leaves crisp edges along the motion
/// and none across it; a subject in focus has both. Judged around each cell, not in fixed blocks,
/// so that the marks don't stop along a block's straight edge.
const CELL: usize = 4;
const REACH: usize = 12;
const CROSSING: f32 = 0.2;

/// A brightness for each step, row by row, and its range, from black to the white level.
pub(crate) struct Plane {
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
    pub range: f32,
    /// The smallest detail that counts, in its values.
    pub least: f32,
}

/// Whether each pixel of the image shown, upright, row by row, is sharp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks {
    pub width: u32,
    pub height: u32,
    pub sharp: Vec<bool>,
}

impl Marks {
    pub fn count(&self) -> usize {
        self.sharp.iter().filter(|s| **s).count()
    }

    /// The marks as a PNG, in `colour` where sharp and clear elsewhere.
    pub fn png(&self, colour: [u8; 3]) -> Result<Vec<u8>, String> {
        let mut image = RgbaImage::new(self.width, self.height);
        for (pixel, sharp) in image.pixels_mut().zip(&self.sharp) {
            if *sharp {
                pixel.0 = [colour[0], colour[1], colour[2], 255];
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).map_err(|e| format!("can't encode the focus marks: {e}"))?;
        Ok(out.into_inner())
    }

    /// Turned as the photo is, from the sensor's orientation.
    fn upright(self, orientation: Orientation) -> Marks {
        if orientation == Orientation::Normal {
            return self;
        }
        let grid = GrayImage::from_raw(self.width, self.height, self.sharp.iter().map(|s| *s as u8).collect()).expect("a mark for each pixel");
        let turned = match orientation {
            Orientation::Rotate90 => image::imageops::rotate90(&grid),
            Orientation::Rotate180 => image::imageops::rotate180(&grid),
            Orientation::Rotate270 => image::imageops::rotate270(&grid),
            Orientation::Normal => grid,
        };
        let (width, height) = turned.dimensions();
        Marks { width, height, sharp: turned.into_raw().into_iter().map(|v| v == 1).collect() }
    }
}

/// A RAW file's marks, from its sensor data, for an image shown at `shown`, upright, over the
/// part of the sensor that image shows, as the clipping marks are.
pub fn of_raw(path: &Path, shown: (u32, u32), level: Level) -> Result<Marks, Problem> {
    let mut image = raw::open(path)?;
    image.unpack().map_err(|e| Problem::DevelopFailed { cause: format!("LibRaw can't read the RAW data of {}: {e}", path.display()) })?;
    let ptr: *mut rsraw_sys::libraw_data_t = image.as_mut();
    let colours = colours_of(ptr);
    // SAFETY: `image` holds the data for the rest of this function, and nothing changes it.
    let data: &rsraw_sys::libraw_data_t = unsafe { &*ptr };
    let Sensor { orientation, area, out } = sensor_for(data, shown)?;
    let plane = plane_of_raw(data, &colours, area).ok_or_else(|| Problem::DevelopFailed { cause: format!("{} has no RAW data PhotoMatch can read for focus", path.display()) })?;
    Ok(marks_at(&plane, (out.0.max(1) as usize, out.1.max(1) as usize), level).upright(orientation))
}

/// The plane's marks at `out`, the size of the image shown, as the sensor sees it: each pixel the
/// sharpest step under it; where the image has more pixels than the sensor has colours, as a
/// 30 MP sensor's 3,360 under a screen's 3,420, each pixel the step it lies on.
fn marks_at(plane: &Plane, out: (usize, usize), level: Level) -> Marks {
    let made = (out.0.min(plane.width), out.1.min(plane.height));
    let sharp = detect(plane, made, level);
    let sharp = if made == out {
        sharp
    } else {
        let column: Vec<usize> = (0..out.0).map(|x| x * made.0 / out.0).collect();
        (0..out.1).flat_map(|y| {
            let row = y * made.1 / out.1;
            column.iter().map(move |x| row * made.0 + x)
        })
        .map(|i| sharp[i])
        .collect()
    };
    Marks { width: out.0 as u32, height: out.1 as u32, sharp }
}

/// The brightness of each full colour in `area` of the visible part of the sensor: each 2 by 2
/// block of a Bayer sensor, each 3 by 3 of an X-Trans one, counted from a multiple of three, or
/// each pixel where LibRaw has a colour for each. Each colour's photosites averaged over their
/// black levels, green weighing twice, and the square root taken.
fn plane_of_raw(data: &rsraw_sys::libraw_data_t, colours: &[[usize; 6]; 24], area: (u32, u32, u32, u32)) -> Option<Plane> {
    let s = &data.sizes;
    let color = &data.color;
    let rawdata = &data.rawdata;
    let (top, left) = (s.top_margin as usize, s.left_margin as usize);
    let (x0, y0, aw, ah) = (area.0 as usize, area.1 as usize, area.2 as usize, area.3 as usize);
    let white = color.maximum as f32;
    let range = (white - color.black as f32).max(1.0).sqrt();
    if !rawdata.raw_image.is_null() {
        let block = if data.idata.filters == 9 { 3 } else { 2 };
        let (bx0, by0) = if block == 3 { (x0.div_ceil(3) * 3, y0.div_ceil(3) * 3) } else { (x0, y0) };
        let (width, height) = ((x0 + aw).saturating_sub(bx0) / block, (y0 + ah).saturating_sub(by0) / block);
        if width < 8 || height < 8 {
            return None;
        }
        let pitch = s.raw_pitch as usize / 2;
        let mut values = vec![0f32; width * height];
        for by in 0..height {
            let mut sums = vec![[0u32; 3]; width];
            let mut counts = vec![[0u8; 3]; width];
            for dy in 0..block {
                let row = by0 + by * block + dy;
                let line = &colours[row % 24];
                // SAFETY: the blocks lie within the area, within the visible part, within the raw
                // image of `raw_height` rows of `pitch` values that LibRaw unpacked.
                let photosites = unsafe { std::slice::from_raw_parts(rawdata.raw_image.add((row + top) * pitch + left + bx0), width * block) };
                for (i, value) in photosites.iter().enumerate() {
                    let col = bx0 + i;
                    let channel = line[col % 6];
                    let c = if channel == 3 { 1 } else { channel };
                    let b = i / block;
                    sums[b][c] += (*value as u32).saturating_sub(black_of(color, channel, row, col));
                    counts[b][c] += 1;
                }
            }
            for (b, (sum, count)) in sums.iter().zip(&counts).enumerate() {
                values[by * width + b] = brightness(sum, count).sqrt();
            }
        }
        Some(Plane { width, height, values, range, least: range * LEAST_DETAIL })
    } else if !rawdata.color4_image.is_null() || !rawdata.color3_image.is_null() {
        // Already a colour for each pixel, as a Sony pixel-shift ARQ or a linear DNG.
        let components = if rawdata.color4_image.is_null() { 3 } else { 4 };
        let pitch = s.raw_pitch as usize / (2 * components);
        if aw < 8 || ah < 8 {
            return None;
        }
        let mut values = vec![0f32; aw * ah];
        for r in 0..ah {
            let row = y0 + r;
            for c in 0..aw {
                let col = x0 + c;
                let at = (row + top) * pitch + left + col;
                // SAFETY: as above, `components` values a pixel.
                let pixel: [u16; 3] = unsafe {
                    if components == 4 {
                        let p = *rawdata.color4_image.add(at);
                        [p[0], p[1], p[2]]
                    } else {
                        *rawdata.color3_image.add(at)
                    }
                };
                let sum = [0, 1, 2].map(|channel| (pixel[channel] as u32).saturating_sub(black_of(color, channel, row, col)));
                values[r * aw + c] = brightness(&sum, &[1, 1, 1]).sqrt();
            }
        }
        Some(Plane { width: aw, height: ah, values, range, least: range * LEAST_DETAIL })
    } else {
        None
    }
}

/// Red, green and blue averaged over their photosites, green weighing twice; those a block lacks
/// left out.
fn brightness(sum: &[u32; 3], count: &[u8; 3]) -> f32 {
    let weights = [1.0, 2.0, 1.0];
    let (mut total, mut weight) = (0.0f32, 0.0f32);
    for c in 0..3 {
        if count[c] > 0 {
            total += weights[c] * sum[c] as f32 / count[c] as f32;
            weight += weights[c];
        }
    }
    if weight > 0.0 { total / weight } else { 0.0 }
}

/// An image file's marks, from its own pixels, upright, at `shown`: its brightness, green
/// weighing twice, as its values stand, already near even in their noise.
pub fn of_image(image: &DynamicImage, shown: (u32, u32), level: Level) -> Marks {
    let rgb = image.to_rgb8();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    let values = rgb.pixels().map(|p| (p.0[0] as f32 + 2.0 * p.0[1] as f32 + p.0[2] as f32) / 4.0).collect();
    let plane = Plane { width, height, values, range: 255.0, least: LEAST_IN_IMAGE };
    let out = (shown.0.max(1) as usize, shown.1.max(1) as usize);
    if width < 8 || height < 8 {
        return Marks { width: out.0 as u32, height: out.1 as u32, sharp: vec![false; out.0 * out.1] };
    }
    marks_at(&plane, out, level)
}

/// Bands of brightness, from black to the white level, each with its own noise.
const BANDS: usize = 24;

/// The noise is found in blocks of this many steps a side, each block's own.
const BLOCK: usize = 16;

/// A band's noise is that of its quietest blocks: this share of them is quieter.
const QUIET_SHARE: f32 = 0.25;

/// A band needs this many blocks for a noise of its own; else it takes the nearest band's.
const LEAST_BLOCKS: usize = 6;

/// The spread of a brightness's noise, from the median size of the steps from each brightness to
/// the next across, which hold two brightnesses' noise: over `MEDIAN_OF_SIZE` and √2, as Donoho
/// and Johnstone's estimate from the finest detail.
fn spread(steps: &mut [f32]) -> f32 {
    if steps.is_empty() {
        return 0.0;
    }
    let middle = steps.len() / 2;
    let (_, median, _) = steps.select_nth_unstable_by(middle, f32::total_cmp);
    *median / MEDIAN_OF_SIZE / std::f32::consts::SQRT_2
}

/// The share `share` of the way up `values`, sorted in place.
fn quantile(values: &mut [f32], share: f32) -> f32 {
    let at = ((values.len() - 1) as f32 * share) as usize;
    let (_, value, _) = values.select_nth_unstable_by(at, f32::total_cmp);
    *value
}

/// The plane's noise for each band of brightness, and for the whole plane. The square root evens
/// out the light's own noise, but not the sensor's read noise, which it makes larger in the
/// shadows: so each band has its own. And a band's steps are mostly texture where the photo is
/// detailed, foliage or fabric: so its noise is that of its quietest blocks, the gaps and the
/// smooth patches every brightness has somewhere, where only the sensor's noise shows.
pub(crate) fn noise_of(plane: &Plane) -> ([f32; BANDS], f32) {
    let (w, h) = (plane.width, plane.height);
    let mut bands: Vec<Vec<f32>> = vec![Vec::new(); BANDS];
    let mut all = Vec::new();
    let mut steps = Vec::with_capacity(BLOCK * BLOCK);
    for by in 0..h / BLOCK {
        for bx in 0..w / BLOCK {
            steps.clear();
            let mut total = 0.0;
            for y in by * BLOCK..(by + 1) * BLOCK {
                let line = &plane.values[y * w + bx * BLOCK..y * w + (bx + 1) * BLOCK];
                total += line.iter().sum::<f32>();
                steps.extend(line.windows(2).map(|pair| (pair[1] - pair[0]).abs()));
            }
            let noise = spread(&mut steps);
            bands[band_of(plane, total / (BLOCK * BLOCK) as f32)].push(noise);
            all.push(noise);
        }
    }
    if all.is_empty() {
        return ([0.0; BANDS], 0.0);
    }
    let whole = quantile(&mut all, QUIET_SHARE);
    let mut found: [Option<f32>; BANDS] = [None; BANDS];
    for (band, blocks) in found.iter_mut().zip(&mut bands) {
        if blocks.len() >= LEAST_BLOCKS {
            *band = Some(quantile(blocks, QUIET_SHARE));
        }
    }
    // A band with too few blocks takes the nearest band's noise, the darker one's if two are as
    // near, as noise doesn't shrink in the shadows.
    let noise = std::array::from_fn(|band| {
        (0..BANDS).flat_map(|d| [band.checked_sub(d), Some(band + d)]).flatten().filter(|b| *b < BANDS).find_map(|b| found[b]).unwrap_or(whole)
    });
    (noise, whole)
}

fn band_of(plane: &Plane, value: f32) -> usize {
    ((value / plane.range.max(1e-6)) * BANDS as f32).clamp(0.0, (BANDS - 1) as f32) as usize
}

/// Whether each pixel of an image of `out`, over the plane, is sharp: any step under it that is.
pub(crate) fn detect(plane: &Plane, out: (usize, usize), level: Level) -> Vec<bool> {
    let (w, h) = (plane.width, plane.height);
    let v = &plane.values;
    let (noise, _) = noise_of(plane);
    let thresholds = noise.map(|n| (n * level.factor()).max(plane.least));
    let steepness = level.steepness();
    // The slope over one step either side against that over three: sharp where the brightness
    // changes faster close up than further out, by a share, and by more than the noise of the
    // brightness there, the darker side's or the lighter's, whichever is noisier.
    let fine = |a: f32, b: f32, far_a: f32, far_b: f32| {
        let (close, far) = ((b - a).abs() / 2.0, (far_b - far_a).abs() / 6.0);
        let threshold = thresholds[band_of(plane, a)].max(thresholds[band_of(plane, b)]);
        close - far > threshold && close > steepness * far
    };
    let mut across = vec![false; w * h];
    let mut down = vec![false; w * h];
    for y in 3..h.saturating_sub(3) {
        for x in 3..w.saturating_sub(3) {
            let i = y * w + x;
            across[i] = fine(v[i - 1], v[i + 1], v[i - 3], v[i + 3]);
            down[i] = fine(v[i - w], v[i + w], v[i - 3 * w], v[i + 3 * w]);
        }
    }
    // An edge across runs down: the row above or below passes too, beside it or one over.
    let mut edges = vec![0u8; w * h];
    for y in 4..h.saturating_sub(4) {
        for x in 4..w.saturating_sub(4) {
            let i = y * w + x;
            let edge_across = across[i] && (x - 1..=x + 1).any(|n| across[i - w - x + n] || across[i + w - x + n]);
            let edge_down = down[i] && (y - 1..=y + 1).any(|n| down[n * w + x - 1] || down[n * w + x + 1]);
            edges[i] = edge_across as u8 | (edge_down as u8) << 1;
        }
    }
    // Each cell's sharp steps, across and down, then the window around each cell, by sums over
    // the cells, judged both ways: not one way only, as motion's are over a whole moving part.
    let (cw, ch) = (w.div_ceil(CELL), h.div_ceil(CELL));
    let mut counts = vec![[0u32; 2]; cw * ch];
    for y in 0..h {
        for x in 0..w {
            let e = edges[y * w + x];
            if e != 0 {
                let count = &mut counts[(y / CELL) * cw + x / CELL];
                count[0] += (e & 1) as u32;
                count[1] += (e >> 1) as u32;
            }
        }
    }
    // Sums from the top left, a row and a column of nothing first.
    let mut sums = vec![[0u32; 2]; (cw + 1) * (ch + 1)];
    for cy in 0..ch {
        for cx in 0..cw {
            for k in 0..2 {
                sums[(cy + 1) * (cw + 1) + cx + 1][k] = counts[cy * cw + cx][k] + sums[cy * (cw + 1) + cx + 1][k] + sums[(cy + 1) * (cw + 1) + cx][k] - sums[cy * (cw + 1) + cx][k];
            }
        }
    }
    let crossing: Vec<bool> = (0..cw * ch)
        .map(|i| {
            let (cx, cy) = (i % cw, i / cw);
            let (x0, y0, x1, y1) = (cx.saturating_sub(REACH), cy.saturating_sub(REACH), (cx + REACH + 1).min(cw), (cy + REACH + 1).min(ch));
            let window = |k: usize| sums[y1 * (cw + 1) + x1][k] + sums[y0 * (cw + 1) + x0][k] - sums[y0 * (cw + 1) + x1][k] - sums[y1 * (cw + 1) + x0][k];
            let (across_n, down_n) = (window(0) as f32, window(1) as f32);
            across_n.min(down_n) >= CROSSING * across_n.max(down_n)
        })
        .collect();
    let mut sharp = vec![false; out.0 * out.1];
    let column_out: Vec<usize> = (0..w).map(|x| x * out.0 / w).collect();
    for y in 0..h {
        let row_out = y * out.1 / h;
        for x in 0..w {
            if edges[y * w + x] != 0 && crossing[(y / CELL) * cw + x / CELL] {
                sharp[row_out * out.0 + column_out[x]] = true;
            }
        }
    }
    sharp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same numbers each run: a small generator, xorshift, and Box and Muller's normal values.
    struct Noise(u64);

    impl Noise {
        fn uniform(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            ((self.0 >> 11) as f64 / (1u64 << 53) as f64) as f32
        }

        fn normal(&mut self) -> f32 {
            let (a, b) = (self.uniform().max(1e-9), self.uniform());
            (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
        }
    }

    const SIZE: usize = 200;

    /// A plane of squares, `step` apart in brightness, blurred by `blur` steps, with noise of
    /// spread `sigma`.
    fn squares(step: f32, blur: f32, sigma: f32) -> Plane {
        let mut values: Vec<f32> = (0..SIZE * SIZE).map(|i| if (i % SIZE / 25 + i / SIZE / 25) % 2 == 0 { 40.0 } else { 40.0 + step }).collect();
        if blur > 0.0 {
            values = blurred(&values, blur);
        }
        let mut noise = Noise(0x9E37_79B9_7F4A_7C15);
        for value in &mut values {
            *value += sigma * noise.normal();
        }
        Plane { width: SIZE, height: SIZE, values, range: 128.0, least: 128.0 * LEAST_DETAIL }
    }

    /// A Gaussian blur of spread `sigma`, across then down.
    fn blurred(values: &[f32], sigma: f32) -> Vec<f32> {
        let reach = (3.0 * sigma).ceil() as isize;
        let kernel: Vec<f32> = (-reach..=reach).map(|k| (-(k * k) as f32 / (2.0 * sigma * sigma)).exp()).collect();
        let pass = |values: &[f32], at: &dyn Fn(usize, isize) -> Option<usize>| -> Vec<f32> {
            (0..values.len())
                .map(|i| {
                    let (mut sum, mut weight) = (0.0, 0.0);
                    for (j, k) in (-reach..=reach).enumerate() {
                        if let Some(n) = at(i, k) {
                            sum += kernel[j] * values[n];
                            weight += kernel[j];
                        }
                    }
                    if weight > 0.0 { sum / weight } else { values[i] }
                })
                .collect()
        };
        let across = pass(values, &|i, k| {
            let x = (i % SIZE) as isize + k;
            (0..SIZE as isize).contains(&x).then(|| (i as isize + k) as usize)
        });
        pass(&across, &|i, k| {
            let y = (i / SIZE) as isize + k;
            (0..SIZE as isize).contains(&y).then(|| (i as isize + k * SIZE as isize) as usize)
        })
    }

    fn share(plane: &Plane, level: Level) -> f64 {
        let sharp = detect(plane, (plane.width, plane.height), level);
        sharp.iter().filter(|s| **s).count() as f64 / sharp.len() as f64
    }

    #[test]
    fn the_noise_is_found_from_the_finest_steps() {
        let plane = squares(0.0, 0.0, 0.5);
        let (_, noise) = noise_of(&plane);
        // The quietest quarter of the blocks: a little under the noise, by the spread of its estimate.
        assert!((0.43..=0.52).contains(&noise), "{noise}");
        let edged = squares(30.0, 0.0, 0.5);
        let (_, noise) = noise_of(&edged);
        assert!((0.43..=0.52).contains(&noise), "the blocks the edges cross are left out: {noise}");
    }

    #[test]
    fn sharp_edges_mark_and_the_same_blurred_or_plain_noise_don_t() {
        let sharp = share(&squares(30.0, 0.0, 0.5), Level::Mid);
        let soft = share(&squares(30.0, 2.5, 0.5), Level::Mid);
        let noise = share(&squares(0.0, 0.0, 0.5), Level::High);
        // Squares of 25 have an edge every 25 steps, across and down: about 2 × 2 / 25 of the steps,
        // a few lost where a straight edge has no other near it.
        assert!(sharp > 0.06, "the sharp edges mark: {sharp}");
        assert!(soft < sharp / 20.0, "blurred by 2.5 steps, they don't: {soft} against {sharp}");
        assert_eq!(noise, 0.0, "noise alone never marks, even at High");
    }

    #[test]
    fn a_noisy_photo_s_noise_isn_t_taken_for_sharpness() {
        // As a photo at a high ISO: the noise eight times as strong, the edges as sharp.
        let noisy = share(&squares(0.0, 0.0, 4.0), Level::High);
        let edges = share(&squares(80.0, 0.0, 4.0), Level::Mid);
        assert_eq!(noisy, 0.0, "its noise alone");
        assert!(edges > 0.05, "and its sharp edges still mark: {edges}");
    }

    #[test]
    fn a_soft_edge_of_high_contrast_in_a_clean_photo_doesn_t_mark() {
        // Its noise next to nothing, as an image with noise reduction: only its shape tells.
        let soft = share(&squares(100.0, 2.0, 0.05), Level::High);
        let sharp = share(&squares(100.0, 0.0, 0.05), Level::Mid);
        assert!(soft < 0.002, "{soft}");
        assert!(sharp > 0.06, "{sharp}");
    }

    #[test]
    fn the_shadows_own_noise_isn_t_taken_for_sharpness() {
        // Clean squares in the light, and in the shadows noise six times as strong, as a sensor's
        // read noise shows after the square root.
        let mut plane = squares(30.0, 0.0, 0.5);
        let mut noise = Noise(7);
        for y in 0..SIZE {
            for x in 0..SIZE / 2 {
                plane.values[y * SIZE + x] = 8.0 + 3.0 * noise.normal();
            }
        }
        let sharp = detect(&plane, (SIZE, SIZE), Level::High);
        let in_shadow = (0..SIZE).map(|y| (0..SIZE / 2 - 4).filter(|x| sharp[y * SIZE + x]).count()).sum::<usize>();
        let in_light = (0..SIZE).map(|y| (SIZE / 2 + 4..SIZE).filter(|x| sharp[y * SIZE + x]).count()).sum::<usize>();
        assert_eq!(in_shadow, 0, "the shadows' noise");
        assert!(in_light > SIZE * SIZE / 40, "the light's sharp edges still mark: {in_light}");
        let (bands, _) = noise_of(&plane);
        assert!(bands[band_of(&plane, 8.0)] > 2.5, "the shadows' band has its own noise: {:?}", bands);
    }

    /// A texture of random brightnesses, blurred across by `along` steps as a blur of motion does, its
    /// detail down left crisp, with noise of spread `sigma`.
    fn texture(along: usize, sigma: f32) -> Plane {
        let mut noise = Noise(42);
        // Patches of 4 by 4 steps, each its own brightness, as a fine texture.
        let patch: Vec<f32> = (0..(SIZE / 4) * (SIZE / 4)).map(|_| 40.0 + 30.0 * noise.uniform()).collect();
        let mut values: Vec<f32> = (0..SIZE * SIZE).map(|i| patch[(i / SIZE / 4) * (SIZE / 4) + i % SIZE / 4]).collect();
        if along > 1 {
            let row: Vec<f32> = values.clone();
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let (from, to) = (x.saturating_sub(along / 2), (x + along / 2).min(SIZE - 1));
                    values[y * SIZE + x] = row[y * SIZE + from..=y * SIZE + to].iter().sum::<f32>() / (to - from + 1) as f32;
                }
            }
        }
        for value in &mut values {
            *value += sigma * noise.normal();
        }
        Plane { width: SIZE, height: SIZE, values, range: 128.0, least: 128.0 * LEAST_DETAIL }
    }

    #[test]
    fn a_texture_in_focus_marks_and_the_same_blurred_by_motion_doesn_t() {
        let still = share(&texture(0, 0.5), Level::Mid);
        let moving = share(&texture(15, 0.5), Level::High);
        assert!(still > 0.1, "in focus and still: {still}");
        assert!(moving < still / 20.0, "blurred across by 15 steps, its crisp streaks down don't count: {moving} against {still}");
    }

    #[test]
    fn a_hot_photosite_doesn_t_mark() {
        let mut plane = squares(0.0, 0.0, 0.5);
        plane.values[100 * SIZE + 100] += 60.0;
        assert_eq!(share(&plane, Level::High), 0.0);
    }

    #[test]
    fn high_marks_the_most_and_low_the_least() {
        let plane = squares(30.0, 1.0, 0.5);
        let (low, mid, high) = (share(&plane, Level::Low), share(&plane, Level::Mid), share(&plane, Level::High));
        assert!(low <= mid && mid <= high && low < high, "{low} {mid} {high}");
    }

    #[test]
    fn each_pixel_shown_takes_the_sharpest_step_under_it() {
        let plane = squares(30.0, 0.0, 0.5);
        let full = detect(&plane, (SIZE, SIZE), Level::Mid);
        let small = detect(&plane, (SIZE / 4, SIZE / 4), Level::Mid);
        assert_eq!(small.len(), SIZE * SIZE / 16);
        for y in 0..SIZE / 4 {
            for x in 0..SIZE / 4 {
                let any = (0..4).any(|dy| (0..4).any(|dx| full[(y * 4 + dy) * SIZE + x * 4 + dx]));
                assert_eq!(small[y * SIZE / 4 + x], any, "at {x}, {y}");
            }
        }
    }

    #[test]
    fn an_image_file_s_sharp_edges_mark_and_its_blurred_ones_don_t() {
        let image = |plane: Plane| {
            let pixels: Vec<u8> = plane.values.iter().flat_map(|v| [v.clamp(0.0, 255.0) as u8; 3]).collect();
            DynamicImage::ImageRgb8(image::RgbImage::from_raw(SIZE as u32, SIZE as u32, pixels).unwrap())
        };
        // The scene blurred, then the sensor's noise, as a photo out of focus is.
        let sharp = of_image(&image(squares(120.0, 0.0, 1.0)), (SIZE as u32, SIZE as u32), Level::Mid);
        let soft = of_image(&image(squares(120.0, 2.5, 1.0)), (SIZE as u32, SIZE as u32), Level::Mid);
        assert!(sharp.count() > SIZE * SIZE / 20, "{}", sharp.count());
        assert!(soft.count() < sharp.count() / 20, "{} against {}", soft.count(), sharp.count());
    }

    #[test]
    fn marks_come_at_the_size_shown_even_where_it_has_more_pixels_than_the_sensor() {
        let plane = squares(30.0, 0.0, 0.5);
        let own = marks_at(&plane, (SIZE, SIZE), Level::Mid);
        let larger = marks_at(&plane, (SIZE * 2, SIZE * 2), Level::Mid);
        assert_eq!((larger.width, larger.height), (SIZE as u32 * 2, SIZE as u32 * 2));
        for y in 0..SIZE * 2 {
            for x in 0..SIZE * 2 {
                assert_eq!(larger.sharp[y * SIZE * 2 + x], own.sharp[(y / 2) * SIZE + x / 2], "at {x}, {y}");
            }
        }
    }

    #[test]
    fn a_point_turns_as_the_photo_does_and_its_box_is_square() {
        let p = upright_point(0.25, 0.75, 1.5, 0);
        assert_eq!((p.x, p.y), (0.25, 0.75));
        assert!((p.width * 1.5 - p.height).abs() < 1e-9, "square on a 3:2 frame: {p:?}");
        let left = upright_point(0.25, 0.75, 1.5, 5);
        assert_eq!((left.x, left.y), (0.75, 0.75), "a quarter left");
        let right = upright_point(0.25, 0.75, 1.5, 6);
        assert_eq!((right.x, right.y), (0.25, 0.25), "a quarter right");
        let half = upright_point(0.25, 0.75, 1.5, 3);
        assert_eq!((half.x, half.y), (0.75, 0.25));
        for flip in [0, 3, 5, 6] {
            let turned = upright_point(0.25, 0.75, 1.5, flip);
            let (x, y, _, _) = sensor_box(&turned, flip);
            assert!((x - 0.25).abs() < 1e-9 && (y - 0.75).abs() < 1e-9, "undone, flip {flip}: {x}, {y}");
        }
    }

    #[test]
    fn the_verdict_is_in_focus_on_sharp_edges_not_on_blurred_ones_and_can_t_tell_on_a_plain_part() {
        // A box over an edge of the squares, at 25, 25 of 200.
        let on_edge = (25.0 / SIZE as f64, 25.0 / SIZE as f64, 0.06, 0.06);
        assert_eq!(verdict(&squares(30.0, 0.0, 0.5), (SIZE, SIZE), Level::Mid, on_edge), Verdict::InFocus);
        assert_eq!(verdict(&squares(30.0, 3.0, 0.5), (SIZE, SIZE), Level::Mid, on_edge), Verdict::NotInFocus);
        assert_eq!(verdict(&squares(0.0, 0.0, 0.5), (SIZE, SIZE), Level::Mid, on_edge), Verdict::CantTell);
    }

    #[test]
    fn a_lone_sharp_edge_the_marks_leave_out_can_t_tell_rather_than_not_in_focus() {
        // Left half dark, right half light: one straight sharp edge down the middle, nothing
        // crossing it, which the crossing rule leaves unmarked.
        let mut noise = Noise(3);
        let values: Vec<f32> = (0..SIZE * SIZE).map(|i| if i % SIZE < SIZE / 2 { 40.0 } else { 70.0 } + 0.5 * noise.normal()).collect();
        let plane = Plane { width: SIZE, height: SIZE, values, range: 128.0, least: 128.0 * LEAST_DETAIL };
        assert_eq!(verdict(&plane, (SIZE, SIZE), Level::Mid, (0.5, 0.5, 0.06, 0.06)), Verdict::CantTell);
    }

    #[test]
    fn marks_turn_as_the_photo_does() {
        let marks = Marks { width: 3, height: 2, sharp: vec![true, false, false, false, false, false] };
        let turned = marks.upright(Orientation::Rotate90);
        assert_eq!((turned.width, turned.height), (2, 3));
        assert_eq!(turned.sharp, vec![false, true, false, false, false, false]);
    }

    #[test]
    fn the_png_is_the_colour_asked_where_sharp_and_clear_elsewhere() {
        let marks = Marks { width: 2, height: 1, sharp: vec![true, false] };
        let png = image::load_from_memory(&marks.png([0, 255, 0]).unwrap()).unwrap().to_rgba8();
        assert_eq!(png.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(png.get_pixel(1, 0).0[3], 0);
    }
}
