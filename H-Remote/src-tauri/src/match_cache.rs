//! Bounded match-result cache for catalog searches (P10-08).
//!
//! Keys include a catalog revision so edits invalidate entries immediately;
//! entries also expire after 24 hours and the map is capped.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use hr_domain::SearchHit;

pub const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
pub const MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone)]
pub struct CachedMatches {
    pub hits: Vec<SearchHit>,
    pub created_at: Instant,
}

#[derive(Debug, Default)]
pub struct MatchCache {
    entries: HashMap<u64, CachedMatches>,
}

impl MatchCache {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a cached entry that is still within the 24-hour window.
    pub fn get(&mut self, key: u64, now: Instant) -> Option<&CachedMatches> {
        let expired = self
            .entries
            .get(&key)
            .is_some_and(|entry| now.saturating_duration_since(entry.created_at) >= CACHE_TTL);
        if expired {
            self.entries.remove(&key);
            return None;
        }
        self.entries.get(&key)
    }

    pub fn insert(&mut self, key: u64, hits: Vec<SearchHit>, now: Instant) {
        self.purge(now);
        if self.entries.len() >= MAX_ENTRIES {
            // Drop the oldest entry so the cache never grows unbounded.
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(key, _)| *key)
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            key,
            CachedMatches {
                hits,
                created_at: now,
            },
        );
    }

    fn purge(&mut self, now: Instant) {
        self.entries
            .retain(|_, entry| now.saturating_duration_since(entry.created_at) < CACHE_TTL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_domain::{SearchHit, TextRange};

    fn hit(id: &str) -> SearchHit {
        SearchHit {
            id: id.to_string(),
            name_ranges: vec![TextRange { start: 0, end: 1 }],
            subtitle_ranges: Vec::new(),
        }
    }

    #[test]
    fn entries_round_trip_and_expire_after_24_hours() {
        let now = Instant::now();
        let mut cache = MatchCache::new();
        cache.insert(1, vec![hit("a")], now);
        assert_eq!(cache.get(1, now).unwrap().hits[0].id, "a");

        let later = now + Duration::from_secs(23 * 60 * 60);
        assert!(cache.get(1, later).is_some());
        let expired = now + CACHE_TTL;
        assert!(cache.get(1, expired).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn the_cache_is_bounded_and_drops_the_oldest_entry() {
        let now = Instant::now();
        let mut cache = MatchCache::new();
        for index in 0..(MAX_ENTRIES as u64 + 5) {
            cache.insert(index, vec![hit("x")], now + Duration::from_millis(index));
        }
        assert_eq!(cache.len(), MAX_ENTRIES);
        assert!(cache.get(0, now).is_none());
        assert!(cache.get(MAX_ENTRIES as u64 + 4, now).is_some());
    }

    #[test]
    fn insert_purges_stale_entries_eagerly() {
        let now = Instant::now();
        let mut cache = MatchCache::new();
        cache.insert(1, Vec::new(), now - CACHE_TTL);
        cache.insert(2, Vec::new(), now);
        assert_eq!(cache.len(), 1);
    }
}
