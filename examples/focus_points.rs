//! What each RAW file records of where the camera focused, as LibRaw reads its maker notes (for
//! PhotoMatch's plan step 2.80): Sony's FocusLocation, Fujifilm's FocusPixel, Olympus's
//! AFPointSelected, Pentax's AF points, and the AF data blocks LibRaw keeps for the others. Prints
//! them beside the image's sizes, and draws each point it can place over the large view into
//! `<out>`, a cross, to look at.
//!
//! cargo run --release --example focus_points -- <out> <folder or file>…

use std::path::PathBuf;

use photo_library::photo::Readers;
use photo_library::thumbs::ThumbCache;
use photo_library::{folder, raw};

const SCREEN: u32 = 2560;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("a folder to write into, then folders or files"));
    std::fs::create_dir_all(&out).expect("the folder to write into");
    let mut paths = Vec::new();
    for arg in args {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            paths.extend(folder::list(&path).expect("a folder").photos.into_iter().map(|e| e.path));
        } else {
            paths.push(path);
        }
    }
    let cache = ThumbCache::new(out.join("cache")).expect("a cache");
    for path in paths.into_iter().filter(|p| raw::is_raw(p)) {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let Ok(mut image) = rsraw::RawImage::open_file(&path) else {
            println!("{name}: can't open");
            continue;
        };
        let ptr: *mut rsraw_sys::libraw_data_t = image.as_mut();
        // SAFETY: LibRaw's own data, opened, read only here.
        let data: &rsraw_sys::libraw_data_t = unsafe { &*ptr };
        let make = cstr(&data.idata.make);
        let model = cstr(&data.idata.model);
        let s = &data.sizes;
        let mn = &data.makernotes;
        println!("== {name}: {make} {model}; raw {}×{}, visible {}×{} at {},{}, flip {}", s.raw_width, s.raw_height, s.width, s.height, s.left_margin, s.top_margin, s.flip);
        // Each maker's own fields; a point as a share of the frame where they place one.
        let mut point: Option<(f64, f64, String)> = None;
        match make.as_str() {
            "Sony" => {
                let so = &mn.sony;
                let fl = so.FocusLocation;
                println!("   Sony FocusLocation {:?}, AFAreaMode {}, AFAreaModeSetting {}, AFPointSelected {}, AFPointsUsed {:?} ({}), AFTracking {}, AFType {}", fl, so.AFAreaMode, so.AFAreaModeSetting, so.AFPointSelected, so.AFPointsUsed, so.nAFPointsUsed, so.AFTracking, so.AFType);
                if fl[0] > 0 && fl[1] > 0 {
                    point = Some((fl[2] as f64 / fl[0] as f64, fl[3] as f64 / fl[1] as f64, "FocusLocation x/w, y/h".into()));
                }
            }
            "Fujifilm" | "FUJIFILM" => {
                let fu = &mn.fuji;
                println!("   Fuji FocusPixel {:?}, FocusMode {}, AFMode {}, FocusSettings {:#x}", fu.FocusPixel, fu.FocusMode, fu.AFMode, fu.FocusSettings);
                // Over the embedded JPEG's size, as the Exif image size gives it for a RAF file (the
                // Focus Points plugin's reading of it).
                let (tw, th) = (data.thumbnail.twidth as f64, data.thumbnail.theight as f64);
                let (tw, th) = if tw < th { (th, tw) } else { (tw, th) };
                println!("   embedded JPEG {}×{}", tw, th);
                if fu.FocusPixel != [0, 0] && tw > 0.0 {
                    point = Some((fu.FocusPixel[0] as f64 / tw, fu.FocusPixel[1] as f64 / th, "FocusPixel over the embedded JPEG's size".into()));
                }
            }
            "Olympus" | "OM Digital Solutions" | "OM Digital" => {
                let ol = &mn.olympus;
                println!("   Olympus AFPointSelected {:?}, AFPoint {}, AFResult {}, AutoFocus {}, FocusMode {:?}", ol.AFPointSelected, ol.AFPoint, ol.AFResult, ol.AutoFocus, ol.FocusMode);
                let a = ol.AFPointSelected;
                if a[0] > 0.0 || a[1] > 0.0 {
                    point = Some((a[0] + a[2] / 2.0, a[1] + a[3] / 2.0, "AFPointSelected's box, its middle".into()));
                }
            }
            "Pentax" | "Ricoh" | "RICOH" | "PENTAX" => {
                let pe = &mn.pentax;
                println!("   Pentax AFPointSelected {:?}, area {}, AFPointsInFocus {:#x} (version {}), AFPointMode {}", pe.AFPointSelected, pe.AFPointSelected_Area, pe.AFPointsInFocus, pe.AFPointsInFocus_version, pe.AFPointMode);
            }
            _ => {}
        }
        let common = &mn.common;
        if common.afcount > 0 {
            for item in common.afdata.iter().take(common.afcount.clamp(0, 4) as usize) {
                println!("   afdata: tag {:#06x}, version {}, {} bytes", item.AFInfoData_tag, item.AFInfoData_version, item.AFInfoData_length);
            }
        }
        // The library's own reading and its verdict, at Mid, drawn as the box an app shows.
        let _ = point;
        drop(image);
        match photo_library::focus::point(&path) {
            Ok(Some(found)) => {
                let Some(shown) = cache.screen(&Readers, &path, SCREEN).ok().and_then(|(bytes, _)| image::load_from_memory(&bytes).ok()) else {
                    println!("   point {found:?}; no large view");
                    continue;
                };
                let mut shown = shown.to_rgb8();
                let (w, h) = (shown.width(), shown.height());
                let started = std::time::Instant::now();
                let verdict = photo_library::focus::judge_raw(&path, (w, h), photo_library::focus::Level::Mid, &found);
                let ms = started.elapsed().as_millis();
                println!("   point at {:.3}, {:.3}, box {:.3} × {:.3}: {verdict:?} in {ms} ms", found.x, found.y, found.width, found.height);
                let colour = match verdict {
                    Ok(photo_library::focus::Verdict::InFocus) => [52, 211, 104],
                    Ok(photo_library::focus::Verdict::NotInFocus) => [255, 176, 32],
                    _ => [255, 255, 255],
                };
                let (x0, y0) = (((found.x - found.width / 2.0) * w as f64) as i64, ((found.y - found.height / 2.0) * h as f64) as i64);
                let (x1, y1) = (((found.x + found.width / 2.0) * w as f64) as i64, ((found.y + found.height / 2.0) * h as f64) as i64);
                for t in 0..4i64 {
                    for x in x0..=x1 {
                        for y in [y0 + t, y1 - t] {
                            if (0..w as i64).contains(&x) && (0..h as i64).contains(&y) {
                                shown.put_pixel(x as u32, y as u32, image::Rgb(colour));
                            }
                        }
                    }
                    for y in y0..=y1 {
                        for x in [x0 + t, x1 - t] {
                            if (0..w as i64).contains(&x) && (0..h as i64).contains(&y) {
                                shown.put_pixel(x as u32, y as u32, image::Rgb(colour));
                            }
                        }
                    }
                }
                let _ = shown.save(out.join(format!("{name}.jpg")));
            }
            Ok(None) => println!("   no focus point recorded"),
            Err(problem) => println!("   {problem:?}"),
        }
    }
}

fn cstr(chars: &[std::os::raw::c_char]) -> String {
    chars.iter().take_while(|c| **c != 0).map(|c| *c as u8 as char).collect()
}
