//! Where a photo's highlights clip and its shadows sink: from a RAW file's own sensor data, never
//! its preview; from an image file's pixels, which are all it has (the library's plan step 2.8,
//! record 0014; PhotoMatch's plan step 2.68).
//!
//! The marks are made at the size of the image they go over, upright, so that they line up with
//! it pixel for pixel. A pixel is a clipped highlight where any photosite under it is at or above
//! the camera's white level, as LibRaw takes it; a deep shadow where its photosites are, on
//! average, `SHADOW_STOPS` stops or more under it, FastRawViewer's default. In an image file, a
//! highlight is a channel at 255, and a shadow a pixel at black.

use std::path::Path;

use image::{DynamicImage, GrayImage, ImageFormat, RgbaImage};

use crate::photo::Orientation;
use crate::problem::Problem;
use crate::raw;

/// How far under the white level a shadow is lost in the sensor's noise: 8 stops, "a standard
/// value for the photographically usable dynamic range", FastRawViewer's default.
pub const SHADOW_STOPS: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mark {
    None = 0,
    Highlight = 1,
    Shadow = 2,
}

/// A mark for each pixel of the image shown, upright, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks {
    pub width: u32,
    pub height: u32,
    pub marks: Vec<Mark>,
}

/// Red over clipped highlights and blue over deep shadows, as Lightroom Classic shows them.
const HIGHLIGHT_COLOUR: [u8; 4] = [255, 0, 0, 255];
const SHADOW_COLOUR: [u8; 4] = [0, 96, 255, 255];

impl Marks {
    pub fn count(&self, mark: Mark) -> usize {
        self.marks.iter().filter(|m| **m == mark).count()
    }

    /// The marks as a PNG, clear where there are none.
    pub fn png(&self) -> Result<Vec<u8>, String> {
        let mut image = RgbaImage::new(self.width, self.height);
        for (pixel, mark) in image.pixels_mut().zip(&self.marks) {
            match mark {
                Mark::Highlight => pixel.0 = HIGHLIGHT_COLOUR,
                Mark::Shadow => pixel.0 = SHADOW_COLOUR,
                Mark::None => {}
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).map_err(|e| format!("can't encode the clipping marks: {e}"))?;
        Ok(out.into_inner())
    }

    /// Turned as the photo is, from the sensor's orientation.
    fn upright(self, orientation: Orientation) -> Marks {
        if orientation == Orientation::Normal {
            return self;
        }
        let grid = GrayImage::from_raw(self.width, self.height, self.marks.iter().map(|m| *m as u8).collect()).expect("a mark for each pixel");
        let turned = match orientation {
            Orientation::Rotate90 => image::imageops::rotate90(&grid),
            Orientation::Rotate180 => image::imageops::rotate180(&grid),
            Orientation::Rotate270 => image::imageops::rotate270(&grid),
            Orientation::Normal => grid,
        };
        let (width, height) = turned.dimensions();
        let marks = turned.into_raw().into_iter().map(|v| match v {
            1 => Mark::Highlight,
            2 => Mark::Shadow,
            _ => Mark::None,
        });
        Marks { width, height, marks: marks.collect() }
    }
}

/// A RAW file's marks, from its sensor data, for an image shown at `shown`, upright. The marks
/// cover the part of the sensor that image shows, as `area_for` finds it.
pub fn of_raw(path: &Path, shown: (u32, u32)) -> Result<Marks, Problem> {
    let mut image = raw::open(path)?;
    image.unpack().map_err(|e| Problem::DevelopFailed { cause: format!("LibRaw can't read the RAW data of {}: {e}", path.display()) })?;
    let ptr: *mut rsraw_sys::libraw_data_t = image.as_mut();
    // Each photosite's colour, by LibRaw, which repeats within 24 rows and 6 columns for a Bayer
    // pattern (8 by 2) and an X-Trans one (6 by 6).
    let mut colours = [[0usize; 6]; 24];
    for (row, line) in colours.iter_mut().enumerate() {
        for (col, cell) in line.iter_mut().enumerate() {
            // SAFETY: LibRaw's own data, unpacked, which it only reads here.
            let c = unsafe { rsraw_sys::libraw_COLOR(ptr, row as i32, col as i32) };
            *cell = c.clamp(0, 3) as usize;
        }
    }
    // SAFETY: `image` holds the data for the rest of this function, and nothing changes it.
    let data: &rsraw_sys::libraw_data_t = unsafe { &*ptr };
    let orientation = raw::orientation(data.sizes.flip);
    // The image shown, as the sensor sees it.
    let (shown_w, shown_h) = if orientation.is_sideways() { (shown.1, shown.0) } else { shown };
    if shown_w == 0 || shown_h == 0 {
        return Err("no size to make the clipping marks at".to_string().into());
    }
    let s = &data.sizes;
    let visible = (s.width as u32, s.height as u32);
    let inset = &s.raw_inset_crops[0];
    let crop = (inset.cwidth > 0 && inset.cheight > 0 && inset.cleft != u16::MAX && inset.ctop != u16::MAX).then(|| {
        let x = (inset.cleft as u32).saturating_sub(s.left_margin as u32).min(visible.0.saturating_sub(1));
        let y = (inset.ctop as u32).saturating_sub(s.top_margin as u32).min(visible.1.saturating_sub(1));
        (x, y, (inset.cwidth as u32).min(visible.0 - x), (inset.cheight as u32).min(visible.1 - y))
    });
    let area = area_for(visible, crop, shown_w as f64 / shown_h as f64);
    let out = (shown_w.min(area.2).max(1) as usize, shown_h.min(area.3).max(1) as usize);

    let color = &data.color;
    let white = color.maximum;
    let pattern = (color.cblack[4] as usize, color.cblack[5] as usize);
    let black = |channel: usize, row: usize, col: usize| -> u32 {
        let mut level = color.black + color.cblack[channel];
        if pattern.0 > 0 && pattern.1 > 0 && 6 + pattern.0 * pattern.1 <= color.cblack.len() {
            level += color.cblack[6 + (row % pattern.0) * pattern.1 + col % pattern.1];
        }
        level
    };

    let rawdata = &data.rawdata;
    let (top, left) = (s.top_margin as usize, s.left_margin as usize);
    let (x0, y0, aw, ah) = (area.0 as usize, area.1 as usize, area.2 as usize, area.3 as usize);
    let column_out: Vec<usize> = (0..aw).map(|c| c * out.0 / aw).collect();
    let mut marks = Accumulator::new(out, ah);
    if !rawdata.raw_image.is_null() {
        let pitch = s.raw_pitch as usize / 2;
        for r in 0..ah {
            let row = y0 + r;
            marks.start_row(r);
            let line = &colours[row % 24];
            // SAFETY: the area lies within the visible part, which lies within the raw image of
            // `raw_height` rows of `pitch` values that LibRaw unpacked.
            let values = unsafe { std::slice::from_raw_parts(rawdata.raw_image.add((row + top) * pitch + left + x0), aw) };
            for (c, value) in values.iter().enumerate() {
                let col = x0 + c;
                marks.add(column_out[c], *value as u32, black(line[col % 6], row, col), white);
            }
        }
    } else if !rawdata.color4_image.is_null() || !rawdata.color3_image.is_null() {
        // Already a colour for each pixel, as a Sony pixel-shift ARQ or a linear DNG.
        let components = if rawdata.color4_image.is_null() { 3 } else { 4 };
        let pitch = s.raw_pitch as usize / (2 * components);
        for r in 0..ah {
            let row = y0 + r;
            marks.start_row(r);
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
                for (channel, value) in pixel.iter().enumerate() {
                    marks.add(column_out[c], *value as u32, black(channel, row, col), white);
                }
            }
        }
    } else {
        return Err(Problem::DevelopFailed { cause: format!("{} has no RAW data PhotoMatch can read for clipping", path.display()) });
    }
    Ok(Marks { width: out.0 as u32, height: out.1 as u32, marks: marks.finish() }.upright(orientation))
}

/// An image file's marks, from its own pixels, upright, at `shown`.
pub fn of_image(image: &DynamicImage, shown: (u32, u32)) -> Marks {
    let rgb = image.to_rgb8();
    let (iw, ih) = (rgb.width() as usize, rgb.height() as usize);
    let out = (shown.0.min(iw as u32).max(1) as usize, shown.1.min(ih as u32).max(1) as usize);
    let column_out: Vec<usize> = (0..iw).map(|c| c * out.0 / iw).collect();
    let mut clipped = vec![false; out.0];
    let mut black = vec![0u32; out.0];
    let mut total = vec![0u32; out.0];
    let mut marks = Vec::with_capacity(out.0 * out.1);
    let mut row_out = 0;
    let finish = |clipped: &mut Vec<bool>, black: &mut Vec<u32>, total: &mut Vec<u32>, marks: &mut Vec<Mark>| {
        for x in 0..out.0 {
            marks.push(if clipped[x] {
                Mark::Highlight
            } else if total[x] > 0 && black[x] * 2 >= total[x] {
                Mark::Shadow
            } else {
                Mark::None
            });
        }
        clipped.iter_mut().for_each(|v| *v = false);
        black.iter_mut().for_each(|v| *v = 0);
        total.iter_mut().for_each(|v| *v = 0);
    };
    for (y, line) in rgb.rows().enumerate() {
        let oy = y * out.1 / ih;
        if oy != row_out {
            finish(&mut clipped, &mut black, &mut total, &mut marks);
            row_out = oy;
        }
        for (x, pixel) in line.enumerate() {
            let ox = column_out[x];
            total[ox] += 1;
            if pixel.0.contains(&255) {
                clipped[ox] = true;
            } else if pixel.0 == [0, 0, 0] {
                black[ox] += 1;
            }
        }
    }
    finish(&mut clipped, &mut black, &mut total, &mut marks);
    Marks { width: out.0 as u32, height: out.1 as u32, marks }
}

/// The part of the sensor an image of `aspect`, width over height, as the sensor sees it, shows,
/// as (left, top, width, height) in the visible area: LibRaw's crop of the camera's own framing
/// when its shape is the image's, as for a 16:9 photo; else the visible area when its shape is,
/// as for a camera whose crop is another shape than its preview; else the visible area cut to the
/// image's shape, centred. Within 1%, the closer shape wins.
pub(crate) fn area_for(visible: (u32, u32), crop: Option<(u32, u32, u32, u32)>, aspect: f64) -> (u32, u32, u32, u32) {
    let off = |w: u32, h: u32| ((w as f64 / h.max(1) as f64) / aspect - 1.0).abs();
    let whole = (0, 0, visible.0, visible.1);
    let mut best: Option<((u32, u32, u32, u32), f64)> = None;
    for candidate in crop.into_iter().chain(std::iter::once(whole)) {
        let miss = off(candidate.2, candidate.3);
        if miss <= 0.01 && best.is_none_or(|(_, b)| miss < b) {
            best = Some((candidate, miss));
        }
    }
    if let Some((area, _)) = best {
        return area;
    }
    let (w, h) = visible;
    if w as f64 / h as f64 > aspect {
        let cut = ((h as f64 * aspect).round() as u32).clamp(1, w);
        ((w - cut) / 2, 0, cut, h)
    } else {
        let cut = ((w as f64 / aspect).round() as u32).clamp(1, h);
        (0, (h - cut) / 2, w, cut)
    }
}

/// Adds photosites up, row by row of the marks, so that only a row of sums is kept at a time.
struct Accumulator {
    out: (usize, usize),
    rows: usize,
    row: usize,
    clipped: Vec<bool>,
    signal: Vec<u64>,
    range: Vec<u64>,
    marks: Vec<Mark>,
}

impl Accumulator {
    fn new(out: (usize, usize), rows: usize) -> Self {
        Self { out, rows, row: 0, clipped: vec![false; out.0], signal: vec![0; out.0], range: vec![0; out.0], marks: Vec::with_capacity(out.0 * out.1) }
    }

    fn start_row(&mut self, row: usize) {
        let row = row * self.out.1 / self.rows;
        if row != self.row {
            self.finish_row();
            self.row = row;
        }
    }

    /// A photosite of `value`, its black level `black`, under the mark at `column`.
    #[inline]
    fn add(&mut self, column: usize, value: u32, black: u32, white: u32) {
        if value >= white {
            self.clipped[column] = true;
        }
        self.signal[column] += value.saturating_sub(black) as u64;
        self.range[column] += white.saturating_sub(black) as u64;
    }

    fn finish_row(&mut self) {
        for x in 0..self.out.0 {
            self.marks.push(if self.clipped[x] {
                Mark::Highlight
            } else if self.range[x] > 0 && self.signal[x] << SHADOW_STOPS <= self.range[x] {
                Mark::Shadow
            } else {
                Mark::None
            });
        }
        self.clipped.iter_mut().for_each(|v| *v = false);
        self.signal.iter_mut().for_each(|v| *v = 0);
        self.range.iter_mut().for_each(|v| *v = 0);
    }

    fn finish(mut self) -> Vec<Mark> {
        self.finish_row();
        // A last row of marks no photosite reached, which rounding can leave, stays clear.
        self.marks.resize(self.out.0 * self.out.1, Mark::None);
        self.marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_area_is_the_camera_s_crop_when_the_image_has_its_shape() {
        // The ZV-E10 sample: a 16:9 preview from a 3:2 sensor, LibRaw's crop 16:9.
        assert_eq!(area_for((6024, 4024), Some((12, 324, 6000, 3376)), 1920.0 / 1080.0), (12, 324, 6000, 3376));
        // The α1's: its full-size preview has the crop's shape.
        assert_eq!(area_for((8660, 5784), Some((12, 12, 8640, 5760)), 8640.0 / 5760.0), (12, 12, 8640, 5760));
    }

    #[test]
    fn the_area_is_the_visible_part_when_the_crop_has_another_shape() {
        // The GH7 sample: a 4:3 preview, LibRaw's crop 3:2.
        assert_eq!(area_for((5792, 4352), Some((8, 252, 5776, 3848)), 1920.0 / 1440.0), (0, 0, 5792, 4352));
        // No crop, as an iPhone's DNG.
        assert_eq!(area_for((4032, 3024), None, 852.0 / 640.0), (0, 0, 4032, 3024));
    }

    #[test]
    fn an_image_of_another_shape_gets_the_visible_part_cut_to_it_centred() {
        assert_eq!(area_for((6000, 4000), None, 16.0 / 9.0), (0, 312, 6000, 3375));
        assert_eq!(area_for((6000, 4000), None, 1.0), (1000, 0, 4000, 4000));
    }

    /// Marks for a made-up sensor of `values`, a row a line, black 512 and white 16383.
    fn marked(values: &[&[u32]], out: (usize, usize)) -> Vec<Mark> {
        let mut marks = Accumulator::new(out, values.len());
        for (r, line) in values.iter().enumerate() {
            marks.start_row(r);
            for (c, value) in line.iter().enumerate() {
                marks.add(c * out.0 / line.len(), *value, 512, 16383);
            }
        }
        marks.finish()
    }

    #[test]
    fn one_photosite_at_the_white_level_marks_its_pixel_a_highlight() {
        let mid = 4000;
        let marks = marked(&[&[16383, mid, mid, mid], &[mid, mid, mid, mid], &[mid, mid, mid, mid], &[mid, mid, mid, mid]], (2, 2));
        assert_eq!(marks, vec![Mark::Highlight, Mark::None, Mark::None, Mark::None]);
    }

    #[test]
    fn a_pixel_whose_photosites_are_8_stops_under_on_average_is_a_shadow() {
        // 8 stops under the range of 15871 above black: 61 above it, at most.
        let deep = 512 + 61;
        let just_over = 512 + 62;
        let marks = marked(&[&[deep, deep, just_over, just_over], &[deep, deep, just_over, just_over]], (2, 1));
        assert_eq!(marks, vec![Mark::Shadow, Mark::None]);
        // At the black level, or under it, as noise can be, is a shadow too.
        assert_eq!(marked(&[&[500, 512]], (1, 1)), vec![Mark::Shadow]);
    }

    #[test]
    fn marks_turn_as_the_photo_does() {
        let marks = Marks { width: 2, height: 1, marks: vec![Mark::Highlight, Mark::Shadow] };
        let turned = marks.clone().upright(Orientation::Rotate90);
        assert_eq!((turned.width, turned.height), (1, 2));
        assert_eq!(turned.marks, vec![Mark::Highlight, Mark::Shadow]);
        let turned = marks.upright(Orientation::Rotate270);
        assert_eq!(turned.marks, vec![Mark::Shadow, Mark::Highlight]);
    }

    #[test]
    fn the_png_is_red_over_highlights_blue_over_shadows_and_clear_elsewhere() {
        let marks = Marks { width: 3, height: 1, marks: vec![Mark::Highlight, Mark::Shadow, Mark::None] };
        let png = image::load_from_memory_with_format(&marks.png().unwrap(), ImageFormat::Png).unwrap().to_rgba8();
        assert_eq!(png.get_pixel(0, 0).0, HIGHLIGHT_COLOUR);
        assert_eq!(png.get_pixel(1, 0).0, SHADOW_COLOUR);
        assert_eq!(png.get_pixel(2, 0).0[3], 0);
    }

    #[test]
    fn an_image_file_s_highlights_are_a_channel_at_255_and_its_shadows_black() {
        let mut image = image::RgbImage::from_pixel(4, 2, image::Rgb([120, 120, 120]));
        image.put_pixel(0, 0, image::Rgb([255, 200, 10]));
        for (x, y) in [(2, 0), (3, 0), (2, 1)] {
            image.put_pixel(x, y, image::Rgb([0, 0, 0]));
        }
        let marks = of_image(&DynamicImage::ImageRgb8(image), (2, 1));
        assert_eq!(marks.marks, vec![Mark::Highlight, Mark::Shadow]);
    }
}
