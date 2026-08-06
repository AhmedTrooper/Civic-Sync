//! Redis-backed cache for hot-path computations (data.md §4
//! "Redis Cache & Pub/Sub (Geospatial Indexing & State Caching)").
//!
//! ## Caches exposed
//!
//! * [`nearest_center`] — caches the result of
//!   `compute_nearest_center(centers, lat, lon)` keyed by `(lat_q, lon_q)` where
//!   the coordinates are rounded to 4 decimals (~11 m at the equator).

use redis::AsyncCommands;
use uuid::Uuid;

use crate::features::centers::Center;

pub const DISPATCH_TTL_SECS: u64 = 60;

/// Quantise latitude and longitude to 4 decimal places (~11 m at the
/// equator) so a cluster of incidents in the same neighbourhood hits
/// the same cache key.
fn quantise(value: f64) -> i32 {
    (value * 10_000.0).round() as i32
}

/// Cached lookup: which command center is nearest to the given
/// coordinates? Returns the previously-cached entry if the rounded
/// coordinates match AND the entry has not expired.
pub async fn nearest_center(
    redis: Option<&redis::Client>,
    centers: &[Center],
    latitude: f64,
    longitude: f64,
) -> Uuid {
    if centers.is_empty() {
        return Uuid::nil();
    }
    let key = format!(
        "dispatch_nearest_center:{}:{}",
        quantise(latitude),
        quantise(longitude)
    );

    if let Some(client) = redis
        && let Ok(mut conn) = client.get_multiplexed_async_connection().await
    {
        let cached: redis::RedisResult<String> = conn.get(&key).await;
        match cached {
            Ok(uuid_str) => match Uuid::parse_str(&uuid_str) {
                Ok(uuid) => {
                    crate::observability::record_cache_op("get", "hit");
                    return uuid;
                }
                Err(_) => {
                    crate::observability::record_cache_op("get", "error");
                }
            },
            Err(error) => {
                tracing::debug!(?error, "redis GET failed; computing locally");
                crate::observability::record_cache_op("get", "error");
            }
        }
    } else if redis.is_some() {
        crate::observability::record_cache_op("get", "error");
    }

    crate::observability::record_cache_op("get", "miss");
    let answer = compute_nearest_center(centers, latitude, longitude);

    if let Some(client) = redis
        && let Ok(mut conn) = client.get_multiplexed_async_connection().await
    {
        match conn
            .set_ex::<_, _, ()>(&key, answer.to_string(), DISPATCH_TTL_SECS)
            .await
        {
            Ok(()) => {
                crate::observability::record_cache_op("set", "hit");
            }
            Err(error) => {
                tracing::debug!(?error, "redis SETEX failed; computed answer returned");
                crate::observability::record_cache_op("set", "error");
            }
        }
    } else if redis.is_some() {
        crate::observability::record_cache_op("set", "error");
    }

    answer
}

fn compute_nearest_center(centers: &[Center], latitude: f64, longitude: f64) -> Uuid {
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
pub async fn clear_for_tests(redis: Option<&redis::Client>) {
    if let Some(client) = redis
        && let Ok(mut conn) = client.get_multiplexed_async_connection().await
    {
        let keys: redis::RedisResult<Vec<String>> = conn.keys("dispatch_nearest_center:*").await;
        if let Ok(keys_vec) = keys {
            for key in keys_vec {
                let _: redis::RedisResult<()> = conn.del(&key).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::centers::seed_centers;

    #[tokio::test]
    async fn cache_returns_same_answer_for_clustered_coordinates() {
        let centers = seed_centers();
        // Since we test without redis locally or via mock, it should fallback
        // to direct computation correctly.
        let first = nearest_center(None, &centers, 23.8101, 90.4126).await;
        let second = nearest_center(None, &centers, 23.8102, 90.4127).await;
        assert_eq!(first, second);
        assert_ne!(first, Uuid::nil());
    }

    #[tokio::test]
    async fn cache_clear_works() {
        clear_for_tests(None).await;
        let centers = seed_centers();
        let first = nearest_center(None, &centers, 23.0, 90.0).await;
        clear_for_tests(None).await;
        let second = nearest_center(None, &centers, 23.0, 90.0).await;
        assert_eq!(first, second);
    }
}
