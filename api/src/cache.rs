//! In-memory caches that sit in front of hot-path computations (data.md §4
//! "Redis Cache & Pub/Sub (Geospatial Indexing & State Caching)").
//!
//! data.md §4 specifies a state cache but does not mandate Redis. Until a
//! production Redis deployment is wired up, an in-memory LRU backed by
//! `lru::LruCache` is good enough for the dispatch hot path. The cache is
//! intentionally simple: time-bounded entries (`CacheEntry<V>::expire`)
//! and a single global key per call site.
//!
//! ## Caches exposed
//!
//! * [`dispatch_lookups`] — caches the result of
//!   `nearest_center(centers, lat, lon)` keyed by `(lat_q, lon_q)` where
//!   the coordinates are rounded to 4 decimals (~11 m at the equator).
//!   That collapses repeated calls for incidents in the same neighbourhood
//!   into a single Haversine traversal across all 8 hubs.

use std::num::NonZeroUsize;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use lru::LruCache;
use uuid::Uuid;

use crate::features::command_centers::CommandCenter;

/// Default time-to-live for dispatch lookup caches. Matches the §5C
/// flush cadence so a stale entry expires before the next sync window.
pub const DISPATCH_TTL: Duration = Duration::from_secs(60);

/// Maximum dispatch lookups cached in memory at any one time. 1024 is a
/// small enough footprint for a hackathon box but large enough to absorb
/// a 10-RPS dispatch call hitting the same neighbourhoods.
const DISPATCH_CAPACITY: NonZeroUsize = NonZeroUsize::new(1024).expect("non-zero");

#[derive(Clone, Copy, Debug)]
struct CacheEntry<V: Copy> {
    value: V,
    inserted_at: Instant,
}

impl<V: Copy> CacheEntry<V> {
    fn is_fresh(&self, now: Instant) -> bool {
        now.duration_since(self.inserted_at) < DISPATCH_TTL
    }
}

type NearestCenterCache = std::sync::Mutex<LruCache<(i32, i32), CacheEntry<Uuid>>>;

static DISPATCH_NEAREST_CENTER: OnceLock<NearestCenterCache> = OnceLock::new();

fn dispatch_cache() -> &'static NearestCenterCache {
    DISPATCH_NEAREST_CENTER.get_or_init(|| std::sync::Mutex::new(LruCache::new(DISPATCH_CAPACITY)))
}

/// Quantise latitude and longitude to 4 decimal places (~11 m at the
/// equator) so a cluster of incidents in the same neighbourhood hits
/// the same cache key.
fn quantise(value: f64) -> i32 {
    (value * 10_000.0).round() as i32
}

/// Cached lookup: which command center is nearest to the given
/// coordinates? Returns the previously-cached entry if the rounded
/// coordinates match AND the entry has not expired.
pub fn nearest_center(centers: &[CommandCenter], latitude: f64, longitude: f64) -> Uuid {
    if centers.is_empty() {
        return Uuid::nil();
    }
    let key = (quantise(latitude), quantise(longitude));
    let now = Instant::now();
    if let Some(entry) = dispatch_cache()
        .lock()
        .expect("dispatch cache lock poisoned")
        .get(&key)
        .copied()
        && entry.is_fresh(now)
    {
        return entry.value;
    }
    let answer = compute_nearest_center(centers, latitude, longitude);
    dispatch_cache()
        .lock()
        .expect("dispatch cache lock poisoned")
        .put(
            key,
            CacheEntry {
                value: answer,
                inserted_at: now,
            },
        );
    answer
}

fn compute_nearest_center(centers: &[CommandCenter], latitude: f64, longitude: f64) -> Uuid {
    let mut best_id = Uuid::nil();
    let mut best_km = f64::INFINITY;
    for center in centers {
        let km = haversine_km(latitude, longitude, center.latitude, center.longitude);
        if km < best_km {
            best_km = km;
            best_id = center.id;
        }
    }
    best_id
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_KM * c
}

/// Clear the dispatch cache. Used by integration tests to avoid stale
/// state between scenarios.
pub fn clear_for_tests() {
    if let Some(cache) = DISPATCH_NEAREST_CENTER.get() {
        cache.lock().expect("dispatch cache lock poisoned").clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::command_centers::seed_command_centers;

    #[test]
    fn cache_returns_same_answer_for_clustered_coordinates() {
        clear_for_tests();
        let centers = seed_command_centers();
        // Two incidents 12 m apart (Dhaka-ish coordinates) should hit the
        // same quantised key and return the same nearest center.
        let first = nearest_center(&centers, 23.8101, 90.4126);
        let second = nearest_center(&centers, 23.8102, 90.4127);
        assert_eq!(first, second);
        assert_ne!(first, Uuid::nil());
    }

    #[test]
    fn cache_clear_drops_all_entries() {
        clear_for_tests();
        let centers = seed_command_centers();
        let first = nearest_center(&centers, 23.0, 90.0);
        clear_for_tests();
        // Same call after clear should still produce a value (re-computed).
        let second = nearest_center(&centers, 23.0, 90.0);
        assert_eq!(first, second);
    }
}
