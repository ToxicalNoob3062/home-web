use super::types::*;
use bazuka::*;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
pub type Cache = Arc<SkmvCache<Query, Response>>;
pub type Tracker = Arc<DashMap<Query, mpsc::Sender<Option<(Response, u32)>>>>;
