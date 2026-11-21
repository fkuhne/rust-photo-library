//! Where a photo was taken, named: its town, its region and its country, offline, from GeoNames'
//! towns of over 1,000 people (record 0010). Behind the feature `places`, as the list adds about
//! 2.7 MB to an app. GeoNames' data is under CC BY 4.0: an app that names places credits it.

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::OnceLock;

use serde::Serialize;

use crate::photo::Position;

/// The list, made by `scripts/places-data.py` from GeoNames' files of 2026-11-21.
static DATA: &[u8] = include_bytes!("../data/places.tsv.gz");

/// A town's circle: this many km for each square root of its people, at least `MIN_RADIUS_KM`. A
/// town of 10,000 covers 1 km around its point, a million 10 km (record 0010).
const KM_PER_ROOT: f64 = 0.01;
const MIN_RADIUS_KM: f64 = 1.0;
/// Outside every circle, the nearest town within this, by name.
const NEAREST_TOWN_KM: f64 = 10.0;
/// Further than this from any town, no place: at sea, or far into a wilderness.
const NEAREST_ANY_KM: f64 = 100.0;
/// The grid the towns are found by, in degrees.
const CELL: f64 = 0.5;
/// About how many towns the list holds, to make room for them once.
const TOWNS: usize = 163_000;

/// A place, in English: a town if one is near, and its region and country.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub town: Option<String>,
    pub region: Option<String>,
    pub country: String,
}

struct Town {
    latitude: f32,
    longitude: f32,
    people: u32,
    /// Its name in `Places::text`.
    name: (u32, u16),
    country: [u8; 2],
    region: u32,
}

struct Places {
    towns: Vec<Town>,
    text: String,
    countries: HashMap<[u8; 2], String>,
    /// Region names, by index; `regions_by_code` finds them by `CC.code`.
    regions: Vec<String>,
    /// The towns of each cell, as a range of `towns`, sorted by cell.
    cells: HashMap<(i32, i32), (u32, u32)>,
    /// The largest circle of any town, in km: how far to look for one that holds a position.
    widest_km: f64,
}

const NO_REGION: u32 = u32::MAX;

fn cell_of(latitude: f64, longitude: f64) -> (i32, i32) {
    ((latitude / CELL).floor() as i32, (longitude / CELL).floor() as i32)
}

fn places() -> &'static Places {
    static PLACES: OnceLock<Places> = OnceLock::new();
    PLACES.get_or_init(|| load(DATA))
}

fn load(data: &[u8]) -> Places {
    // Line by line from the compressed list, not unpacked whole first, and the towns' room made
    // once: the peak stays near what's kept (record 0010).
    let lines = std::io::BufReader::new(flate2::read::GzDecoder::new(data)).lines();
    let mut text = String::with_capacity(1 << 21);
    let mut countries = HashMap::new();
    let mut regions = Vec::new();
    let mut regions_by_code = HashMap::new();
    let mut towns = Vec::with_capacity(TOWNS);
    for line in lines {
        let line = line.expect("the place list, built in, reads");
        let mut columns = line.split('\t');
        match columns.next() {
            Some("C") => {
                if let (Some(code), Some(name)) = (columns.next(), columns.next()) {
                    if let Ok(code) = code.as_bytes().try_into() {
                        countries.insert(code, name.to_string());
                    }
                }
            }
            Some("R") => {
                if let (Some(code), Some(name)) = (columns.next(), columns.next()) {
                    regions_by_code.insert(code.to_string(), regions.len() as u32);
                    regions.push(name.to_string());
                }
            }
            Some("T") => {
                let (Some(name), Some(latitude), Some(longitude), Some(country), Some(region), Some(people)) =
                    (columns.next(), columns.next(), columns.next(), columns.next(), columns.next(), columns.next())
                else {
                    continue;
                };
                let (Ok(latitude), Ok(longitude), Ok(code)) = (latitude.parse(), longitude.parse(), <[u8; 2]>::try_from(country.as_bytes())) else { continue };
                let at = text.len() as u32;
                text.push_str(name);
                towns.push(Town {
                    latitude,
                    longitude,
                    people: people.parse().unwrap_or(0),
                    name: (at, name.len().min(u16::MAX as usize) as u16),
                    country: code,
                    region: regions_by_code.get(&format!("{country}.{region}")).copied().unwrap_or(NO_REGION),
                });
            }
            _ => {}
        }
    }
    // In place, as the order within a cell doesn't matter: no second buffer of the towns.
    towns.sort_unstable_by_key(|t| cell_of(t.latitude as f64, t.longitude as f64));
    let mut cells = HashMap::new();
    let mut start = 0;
    while start < towns.len() {
        let cell = cell_of(towns[start].latitude as f64, towns[start].longitude as f64);
        let end = start + towns[start..].iter().take_while(|t| cell_of(t.latitude as f64, t.longitude as f64) == cell).count();
        cells.insert(cell, (start as u32, end as u32));
        start = end;
    }
    let widest_km = towns.iter().map(|t| radius_km(t.people)).fold(MIN_RADIUS_KM, f64::max);
    Places { towns, text, countries, regions, cells, widest_km }
}

fn radius_km(people: u32) -> f64 {
    (KM_PER_ROOT * (people as f64).sqrt()).max(MIN_RADIUS_KM)
}

/// The distance between two points on the Earth, in km, by the haversine formula.
fn distance_km(a: Position, latitude: f64, longitude: f64) -> f64 {
    let (la1, lo1, la2, lo2) = (a.latitude.to_radians(), a.longitude.to_radians(), latitude.to_radians(), longitude.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6371.0 * h.sqrt().min(1.0).asin()
}

impl Places {
    /// The towns within `km` of `at`, each with its distance.
    fn near(&self, at: Position, km: f64) -> Vec<(f64, &Town)> {
        let span_lat = km / 111.0;
        let span_lon = (km / (111.0 * at.latitude.to_radians().cos().max(0.01))).min(180.0);
        let (lat_from, lon_from) = cell_of(at.latitude - span_lat, at.longitude - span_lon);
        let (lat_to, lon_to) = cell_of(at.latitude + span_lat, at.longitude + span_lon);
        let lon_cells = (360.0 / CELL) as i32;
        let mut found = Vec::new();
        for lat_cell in lat_from..=lat_to {
            for lon_cell in lon_from..=lon_to.min(lon_from + lon_cells - 1) {
                // Across the date line, the cells on the other side.
                let wrapped = (lon_cell + lon_cells / 2).rem_euclid(lon_cells) - lon_cells / 2;
                let Some(&(start, end)) = self.cells.get(&(lat_cell, wrapped)) else { continue };
                for town in &self.towns[start as usize..end as usize] {
                    let d = distance_km(at, town.latitude as f64, town.longitude as f64);
                    if d <= km {
                        found.push((d, town));
                    }
                }
            }
        }
        found
    }

    fn name(&self, town: &Town) -> &str {
        &self.text[town.name.0 as usize..town.name.0 as usize + town.name.1 as usize]
    }
}

/// The place of `at`: the largest town whose circle holds it, or else the nearest town within
/// 10 km; and the region and country of that town, or else of the nearest within 100 km. `None`
/// further than that from any town (record 0010).
pub fn place_of(at: Position) -> Option<Place> {
    let places = places();
    let near = places.near(at, places.widest_km.max(NEAREST_ANY_KM));
    let nearest = near.iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
    if nearest.0 > NEAREST_ANY_KM {
        return None;
    }
    let holding = near.iter().filter(|(d, town)| *d <= radius_km(town.people)).max_by(|a, b| a.1.people.cmp(&b.1.people).then(b.0.total_cmp(&a.0)));
    let town = holding.or(Some(nearest).filter(|(d, _)| *d <= NEAREST_TOWN_KM)).map(|(_, town)| *town);
    let anchor = town.unwrap_or(nearest.1);
    let country = places.countries.get(&anchor.country).cloned().unwrap_or_else(|| String::from_utf8_lossy(&anchor.country).into_owned());
    let region = (anchor.region != NO_REGION).then(|| places.regions[anchor.region as usize].clone());
    Some(Place { town: town.map(|t| places.name(t).to_string()), region, country })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(latitude: f64, longitude: f64) -> Place {
        place_of(Position::new(latitude, longitude).unwrap()).expect("a place")
    }

    fn town(latitude: f64, longitude: f64) -> String {
        at(latitude, longitude).town.unwrap_or_default()
    }

    /// The seven spots of record 0010, named as a person would name them.
    #[test]
    fn towns_are_named_by_their_circles() {
        // Near the Golden Gate: in San Francisco, though Sausalito's point is nearer.
        assert_eq!(at(37.808167, -122.475722), Place { town: Some("San Francisco".into()), region: Some("California".into()), country: "United States".into() });
        assert_eq!(town(48.8867, 2.3431), "Paris", "Montmartre");
        assert_eq!(town(48.8049, 2.1204), "Versailles", "the palace, not a district of Paris");
        assert_eq!(town(37.8590, -122.4852), "Sausalito");
        assert_eq!(town(40.7061, -73.9969), "New York City", "the Brooklyn Bridge");
        assert_eq!(town(37.7456, -119.5936), "Yosemite Valley", "the nearest town, outside its circle");
    }

    #[test]
    fn far_from_any_town_the_region_and_the_country_name_it() {
        let lagoon = at(64.048, -16.179);
        assert_eq!((lagoon.town, lagoon.country.as_str()), (None, "Iceland"), "Jökulsárlón, 52 km from Höfn");
        assert!(lagoon.region.is_some());
    }

    #[test]
    fn at_sea_there_is_no_place() {
        assert_eq!(place_of(Position::new(30.0, -40.0).unwrap()), None, "the middle of the Atlantic");
    }

    #[test]
    fn across_the_date_line_towns_are_found() {
        // Taveuni, Fiji, on the 180th meridian: towns on both sides.
        assert_eq!(at(-16.8, 179.99).country, "Fiji");
        assert_eq!(at(-16.8, -179.99).country, "Fiji");
    }
}
