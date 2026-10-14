// Copyright 2026 Sarthak Siddhpura
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{HashMap, VecDeque};

use super::types::QueryResults;

pub const DEFAULT_QUERY_CACHE_LIMIT: usize = 10;

#[derive(Debug, Clone)]
pub struct QueryCache {
    capacity: usize,
    entries: HashMap<String, QueryResults>,
    order: VecDeque<String>,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(DEFAULT_QUERY_CACHE_LIMIT)
    }
}

impl QueryCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
        }
    }

    pub fn get(&mut self, sql: &str) -> Option<QueryResults> {
        if self.capacity == 0 {
            return None;
        }
        if let Some(results) = self.entries.get(sql) {
            if let Some(pos) = self.order.iter().position(|k| k == sql) {
                self.order.remove(pos);
            }
            self.order.push_back(sql.to_string());
            let mut cached = results.clone();
            cached.duration_ms = 0;
            Some(cached)
        } else {
            None
        }
    }

    pub fn insert(&mut self, sql: String, results: QueryResults) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.contains_key(&sql) {
            if let Some(pos) = self.order.iter().position(|k| k == &sql) {
                self.order.remove(pos);
            }
        } else if self.entries.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.entries.remove(&oldest);
        }
        self.order.push_back(sql.clone());
        self.entries.insert(sql, results);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_results(tag: &str) -> QueryResults {
        QueryResults {
            columns: vec![],
            data: vec![vec![tag.to_string()]],
            duration_ms: 100,
        }
    }

    #[test]
    fn test_cache_hit_and_miss() {
        let mut cache = QueryCache::new(2);
        assert_eq!(cache.len(), 0);
        assert!(cache.get("SELECT 1").is_none());

        cache.insert("SELECT 1".to_string(), dummy_results("res1"));
        assert_eq!(cache.len(), 1);

        let hit = cache.get("SELECT 1").expect("should hit");
        assert_eq!(hit.data[0][0], "res1");
        assert_eq!(hit.duration_ms, 0); // Cached duration reported as 0ms
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = QueryCache::new(2);
        cache.insert("Q1".to_string(), dummy_results("1"));
        cache.insert("Q2".to_string(), dummy_results("2"));

        // Access Q1 to make Q2 LRU
        assert!(cache.get("Q1").is_some());

        // Insert Q3, should evict Q2
        cache.insert("Q3".to_string(), dummy_results("3"));

        assert_eq!(cache.len(), 2);
        assert!(cache.get("Q1").is_some());
        assert!(cache.get("Q2").is_none());
        assert!(cache.get("Q3").is_some());
    }

    #[test]
    fn test_cache_zero_capacity() {
        let mut cache = QueryCache::new(0);
        cache.insert("Q1".to_string(), dummy_results("1"));
        assert_eq!(cache.len(), 0);
        assert!(cache.get("Q1").is_none());
    }

    #[test]
    fn test_cache_update_existing() {
        let mut cache = QueryCache::new(2);
        cache.insert("Q1".to_string(), dummy_results("old"));
        cache.insert("Q1".to_string(), dummy_results("new"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("Q1").unwrap().data[0][0], "new");
    }
}
