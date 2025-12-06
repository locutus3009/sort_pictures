//! GPS processing utilities for sort_pictures.
//!
//! This module handles extraction of GPS coordinates from EXIF metadata
//! and matching photos to configured places based on geographic proximity.

use geo::{Distance, Geodesic, Point};
use nom_exif::{GPSInfo, LatLng, URational};

use crate::config::Place;

/// GPS coordinates in decimal degrees (WGS84).
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct GpsCoordinates {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

#[allow(dead_code)]
impl GpsCoordinates {
    /// Creates new GPS coordinates from latitude and longitude.
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }

    /// Converts to a geo Point for distance calculations.
    fn to_point(self) -> Point<f64> {
        Point::new(self.lon, self.lat) // geo crate uses (longitude, latitude) order
    }
}

/// Converts a URational (fraction) to f64.
fn urational_to_f64(rational: &URational) -> f64 {
    rational.0 as f64 / rational.1 as f64
}

/// Converts EXIF latitude/longitude (degrees/minutes/seconds) to decimal degrees.
fn latlng_to_decimal_degrees(latlng: &LatLng) -> f64 {
    let degrees = urational_to_f64(&latlng.0);
    let minutes = urational_to_f64(&latlng.1);
    let seconds = urational_to_f64(&latlng.2);

    degrees
        + minutes / 60.0
        + if seconds > 1.0 {
            seconds / 3600.0
        } else {
            seconds / 60.0
        }
}

/// Converts GPSInfo from EXIF to a geo Point for distance calculations.
pub fn gps_to_point(gps: &GPSInfo) -> Point<f64> {
    let mut lat = latlng_to_decimal_degrees(&gps.latitude);
    let mut lon = latlng_to_decimal_degrees(&gps.longitude);

    // Apply hemisphere references
    if gps.latitude_ref == 'S' {
        lat = -lat;
    }
    if gps.longitude_ref == 'W' {
        lon = -lon;
    }

    Point::new(lon, lat) // geo crate uses (longitude, latitude) order
}

/// Converts GPSInfo from EXIF to GpsCoordinates.
#[allow(dead_code)]
pub fn gps_info_to_coordinates(gps: &GPSInfo) -> GpsCoordinates {
    let mut lat = latlng_to_decimal_degrees(&gps.latitude);
    let mut lon = latlng_to_decimal_degrees(&gps.longitude);

    // Apply hemisphere references
    if gps.latitude_ref == 'S' {
        lat = -lat;
    }
    if gps.longitude_ref == 'W' {
        lon = -lon;
    }

    GpsCoordinates::new(lat, lon)
}

/// Calculates the geodesic distance between GPS coordinates and a target point.
///
/// # Arguments
/// * `gps` - GPS info from EXIF metadata
/// * `target_coords` - Target coordinates as (latitude, longitude)
///
/// # Returns
/// Distance in meters.
pub fn calculate_distance(gps: &GPSInfo, target_coords: &(f64, f64)) -> f64 {
    let gps_point = gps_to_point(gps);
    let target = Point::new(target_coords.1, target_coords.0); // (lon, lat)

    Geodesic.distance(gps_point, target)
}

/// Calculates the geodesic distance between two GPS coordinate points.
///
/// # Arguments
/// * `point1` - First GPS coordinates
/// * `point2` - Second GPS coordinates
///
/// # Returns
/// Distance in kilometers.
#[allow(dead_code)]
pub fn distance_km(point1: &GpsCoordinates, point2: &GpsCoordinates) -> f64 {
    let p1 = point1.to_point();
    let p2 = point2.to_point();

    Geodesic.distance(p1, p2) / 1000.0
}

/// Result of place matching, containing the matched place and distance.
pub struct PlaceMatch<'a> {
    /// The matched place configuration.
    pub place: &'a Place,
    /// Distance from photo location to place center in kilometers.
    pub distance_km: f64,
}

/// Finds the first matching place for given GPS coordinates.
///
/// Iterates through configured places and returns the first one where
/// the photo's GPS coordinates fall within the place's radius.
///
/// # Arguments
/// * `gps` - GPS info extracted from photo EXIF
/// * `places` - Slice of configured places to check against
///
/// # Returns
/// `Some(PlaceMatch)` if coordinates match a place, `None` otherwise.
pub fn find_matching_place<'a>(gps: &GPSInfo, places: &'a [Place]) -> Option<PlaceMatch<'a>> {
    for place in places {
        let pos: (f64, f64) = (place.lat, place.lon);
        let distance = calculate_distance(gps, &pos) / 1000.0; // Convert to km

        if distance < place.radius {
            return Some(PlaceMatch {
                place,
                distance_km: distance,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gps_coordinates_new() {
        let coords = GpsCoordinates::new(51.027290, 13.773699);
        assert!((coords.lat - 51.027290).abs() < 0.0001);
        assert!((coords.lon - 13.773699).abs() < 0.0001);
    }

    #[test]
    fn test_distance_km_same_point() {
        let p1 = GpsCoordinates::new(51.027290, 13.773699);
        let p2 = GpsCoordinates::new(51.027290, 13.773699);
        let dist = distance_km(&p1, &p2);
        assert!(dist < 0.001); // Should be essentially zero
    }

    #[test]
    fn test_distance_km_different_points() {
        // Roughly Dresden to Berlin (~200km)
        let dresden = GpsCoordinates::new(51.05, 13.74);
        let berlin = GpsCoordinates::new(52.52, 13.40);
        let dist = distance_km(&dresden, &berlin);
        // Should be approximately 165-170km
        assert!(dist > 150.0 && dist < 200.0);
    }
}
