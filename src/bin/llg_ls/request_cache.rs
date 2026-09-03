//! Request-level memoization for read-only LSP feature queries.
//!
//! Read-only requests (`textDocument/definition`, `hover`, `references`, and
//! the isolated open-buffer `semanticTokens/full` parse) are pure functions of
//! explicit inputs: the query parameters plus the immutable per-root analysis
//! snapshot they read.  This module bounds and stores those results so a
//! repeat identical request is served from memory instead of recomputing.
//!
//! Correctness model — no TTLs, only input-derived keys:
//!
//! * Navigation results depend on `(query, analysis snapshot)`.  Every commit
//!   that replaces a root's `last_good` snapshot stamps it with a fresh
//!   process-global [`analysis_epoch`] (see `commit_job`), so any input change
//!   that flows through an analysis commit (buffer edits, file saves via
//!   watched-file events, config hot reload incl. `[compile]` defines /
//!   param_overrides) changes the epoch and invalidates the affected entries
//!   structurally.  Entries keyed by a superseded epoch can never be served
//!   again; they age out through the LRU bound.
//! * Open-buffer token streams depend on `(buffer text, -D defines)` only
//!   (Surelog `-parseonly` over one staged copy); both are folded into the
//!   key, so a `[compile] defines` hot reload or any edit misses the cache.
//!
//! The cache is thread-safe (`Mutex`) and bounded (small LRU); values are
//! cheap clones (`Location`, `Hover`, token vectors), so entries never pin a
//! retired `Analysis`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Default entry bound for navigation-result caches.
pub(crate) const NAVIGATION_CACHE_CAPACITY: usize = 128;

/// Entry bound for open-buffer semantic-token payloads (each payload can be
/// large for big buffers, so the bound stays small).
pub(crate) const TOKEN_CACHE_CAPACITY: usize = 8;

static NEXT_ANALYSIS_EPOCH: AtomicU64 = AtomicU64::new(1);

/// A fresh process-global epoch identifying one immutable analysis snapshot.
///
/// Called exactly where a backend commits a replacement snapshot; two calls
/// never yield the same value, so a key carrying an older epoch cannot match a
/// newer snapshot.
pub(crate) fn next_analysis_epoch() -> u64 {
    NEXT_ANALYSIS_EPOCH.fetch_add(1, Ordering::Relaxed)
}

/// Which feature computation a key identifies, with the request shape folded
/// in (parameters that change the answer must change the key).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RequestKind {
    Definition,
    Hover,
    References { include_declaration: bool },
}

/// Cache key for one read-only navigation request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RequestKey {
    pub(crate) kind: RequestKind,
    pub(crate) uri: String,
    pub(crate) line: u32,
    pub(crate) character: u32,
    /// Identity of the analysis snapshot the result was computed from.
    pub(crate) analysis_epoch: u64,
}

impl RequestKey {
    pub(crate) fn new(
        kind: RequestKind,
        uri: impl Into<String>,
        line: u32,
        character: u32,
        analysis_epoch: u64,
    ) -> Self {
        Self {
            kind,
            uri: uri.into(),
            line,
            character,
            analysis_epoch,
        }
    }
}

/// Hit/miss counters surfaced through `llg/dumpTokens` for observability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CacheStats {
    pub(crate) hits: u64,
    pub(crate) misses: u64,
}

impl CacheStats {
    pub(crate) fn add(&mut self, other: &CacheStats) {
        self.hits += other.hits;
        self.misses += other.misses;
    }
}

/// Bounded LRU memoization store.
///
/// Entries carry a monotonically increasing recency stamp refreshed on every
/// hit; insertion past capacity evicts the least recently used entry.  A
/// single global counter orders recency across all instances, which keeps
/// eviction deterministic under concurrent access.
pub(crate) struct MemoCache<K, V> {
    inner: Mutex<Inner<K, V>>,
}

struct Inner<K, V> {
    capacity: usize,
    map: HashMap<K, (V, u64)>,
    hits: u64,
    misses: u64,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> MemoCache<K, V> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                capacity: capacity.max(1),
                map: HashMap::new(),
                hits: 0,
                misses: 0,
            }),
        }
    }

    /// Clone of the stored value for `key`, refreshing its recency.
    pub(crate) fn get(&self, key: &K) -> Option<V> {
        let stamp = next_recency_stamp();
        let mut inner = self.lock();
        let hit = inner.map.get_mut(key).map(|(value, seen)| {
            *seen = stamp;
            value.clone()
        });
        match hit {
            Some(value) => {
                inner.hits += 1;
                Some(value)
            }
            None => {
                inner.misses += 1;
                None
            }
        }
    }

    /// Insert or refresh `key`; evicts the least recently used entry when past
    /// capacity.  A fresh insert counts as a miss only if `get` was not called;
    /// callers use [`Self::get`] first, so nothing is double-counted here.
    pub(crate) fn put(&self, key: K, value: V) {
        let mut inner = self.lock();
        let stamp = next_recency_stamp();
        if inner.map.len() >= inner.capacity && !inner.map.contains_key(&key) {
            if let Some(oldest) = inner
                .map
                .iter()
                .min_by_key(|(_, (_, seen))| *seen)
                .map(|(key, _)| key.clone())
            {
                inner.map.remove(&oldest);
            }
        }
        inner.map.insert(key, (value, stamp));
    }

    pub(crate) fn len(&self) -> usize {
        self.lock().map.len()
    }

    pub(crate) fn stats(&self) -> CacheStats {
        let inner = self.lock();
        CacheStats {
            hits: inner.hits,
            misses: inner.misses,
        }
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, key: &K) -> bool {
        self.lock().map.contains_key(key)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<K, V>> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn next_recency_stamp() -> u64 {
    // Relaxed is enough: the stamp only needs to be unique and monotone per
    // observation, never a synchronization point.
    NEXT_RECENCY_STAMP.fetch_add(1, Ordering::Relaxed)
}

static NEXT_RECENCY_STAMP: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn nav_key(uri: &str, line: u32, col: u32, epoch: u64) -> RequestKey {
        RequestKey::new(RequestKind::Definition, uri, line, col, epoch)
    }

    #[test]
    fn unchanged_inputs_repeat_the_same_key() {
        let a = nav_key("file:///p.sv", 3, 7, 11);
        let b = nav_key("file:///p.sv", 3, 7, 11);
        assert_eq!(a, b);
    }

    #[test]
    fn any_input_change_changes_the_key() {
        let base = nav_key("file:///p.sv", 3, 7, 11);
        assert_ne!(base, nav_key("file:///p.sv", 3, 8, 11), "column");
        assert_ne!(base, nav_key("file:///p.sv", 4, 7, 11), "line");
        assert_ne!(base, nav_key("file:///q.sv", 3, 7, 11), "uri");
        assert_ne!(base, nav_key("file:///p.sv", 3, 7, 12), "analysis epoch");
        assert_ne!(
            base,
            RequestKey::new(RequestKind::Hover, "file:///p.sv", 3, 7, 11),
            "request kind"
        );
        assert_ne!(
            base,
            RequestKey::new(
                RequestKind::References {
                    include_declaration: true
                },
                "file:///p.sv",
                3,
                7,
                11
            ),
            "references parameter"
        );
        assert_ne!(
            RequestKey::new(
                RequestKind::References {
                    include_declaration: true
                },
                "file:///p.sv",
                3,
                7,
                11
            ),
            RequestKey::new(
                RequestKind::References {
                    include_declaration: false
                },
                "file:///p.sv",
                3,
                7,
                11
            ),
            "include_declaration flag"
        );
    }

    #[test]
    fn repeat_get_is_a_hit_and_put_alone_stays_unread() {
        let cache: MemoCache<RequestKey, Option<String>> =
            MemoCache::new(NAVIGATION_CACHE_CAPACITY);
        assert_eq!(cache.get(&nav_key("u", 0, 0, 1)), None);
        assert_eq!(cache.stats().misses, 1);

        cache.put(nav_key("u", 0, 0, 1), Some("loc".to_owned()));
        assert_eq!(
            cache.get(&nav_key("u", 0, 0, 1)),
            Some(Some("loc".to_owned()))
        );
        assert_eq!(cache.stats().hits, 1);

        // A negative result (None) is cached like any other value.
        cache.put(nav_key("u", 1, 0, 1), None);
        assert_eq!(cache.get(&nav_key("u", 1, 0, 1)), Some(None));
    }

    #[test]
    fn epoch_bump_invalidates_without_touching_old_entry_storage() {
        let cache: MemoCache<RequestKey, Option<String>> =
            MemoCache::new(NAVIGATION_CACHE_CAPACITY);
        let old_epoch = next_analysis_epoch();
        cache.put(nav_key("u", 0, 0, old_epoch), Some("old".to_owned()));
        let new_epoch = next_analysis_epoch();
        assert_ne!(old_epoch, new_epoch);
        assert_eq!(cache.get(&nav_key("u", 0, 0, new_epoch)), None);
        assert_eq!(
            cache.get(&nav_key("u", 0, 0, old_epoch)),
            Some(Some("old".to_owned())),
            "the superseded entry ages out via LRU only"
        );
    }

    #[test]
    fn eviction_keeps_the_bound_and_recency_order() {
        let cache: MemoCache<u64, u64> = MemoCache::new(4);
        for i in 0..4 {
            cache.put(i, i);
        }
        assert_eq!(cache.len(), 4);

        // Touch key 0 so it becomes most recently used; then overflow with 4:
        // key 1 (least recently used) must be evicted, not key 0.
        assert!(cache.contains(&0));
        let _ = cache.get(&0);
        cache.put(4, 4);
        assert_eq!(cache.len(), 4);
        assert!(!cache.contains(&1), "LRU victim expected");
        assert!(cache.contains(&0), "recently used entry survives");
        assert!(cache.contains(&4));
        assert_eq!(cache.get(&1), None);

        // The bound holds under further churn.
        for i in 10..40 {
            cache.put(i, i);
        }
        assert!(
            cache.len() <= 4,
            "cache grew past capacity: {}",
            cache.len()
        );
    }

    #[test]
    fn replacing_an_entry_releases_the_stale_value() {
        #[derive(Clone)]
        struct DropProbe(Arc<AtomicUsize>);

        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let cache = MemoCache::new(1);
        cache.put("same-key", DropProbe(Arc::clone(&drops)));
        cache.put("same-key", DropProbe(Arc::clone(&drops)));

        assert_eq!(cache.len(), 1);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn concurrent_churn_never_exceeds_capacity() {
        const CAPACITY: usize = 8;
        let cache = Arc::new(MemoCache::new(CAPACITY));
        let workers = (0..8)
            .map(|worker| {
                let cache = Arc::clone(&cache);
                std::thread::spawn(move || {
                    for item in 0..100 {
                        let key = worker * 100 + item;
                        cache.put(key, key);
                        assert!(cache.len() <= CAPACITY);
                        let _ = cache.get(&key);
                    }
                })
            })
            .collect::<Vec<_>>();

        for worker in workers {
            worker.join().expect("cache worker panicked");
        }

        assert!(cache.len() <= CAPACITY);
    }

    #[test]
    fn stats_track_hits_and_misses() {
        let cache: MemoCache<u64, u64> = MemoCache::new(2);
        cache.put(1, 1);
        let before = cache.stats();
        let _ = cache.get(&1);
        let _ = cache.get(&2);
        let after = cache.stats();
        assert_eq!(after.hits, before.hits + 1);
        assert_eq!(after.misses, before.misses + 1);
    }
}
