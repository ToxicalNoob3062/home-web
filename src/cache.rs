use std::sync::Arc;

use super::types::*;
use bazuka::*;

pub struct Cache {
    cache: SkmvCache<Query, Response>,
}

impl Cache {
    pub fn new(config: SkmvConfig) -> Self {
        Cache {
            cache: SkmvCache::new(config),
        }
    }

    pub async fn insert(&mut self, query: Query, response: Response, ttl: u32) {
        self.cache.insert(query, response, ttl).await;
    }

    pub async fn get(&self, query: &Query) -> Vec<Arc<Response>> {
        self.cache.get(query).await
    }
}
