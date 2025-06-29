use super::cache::Cache;
use super::types::*;
use super::listener::Listener;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

struct TimeBomb (mpsc::Sender<Option<Response>>, mpsc::Receiver<Option<Response>>);

impl TimeBomb {
    pub fn new(duration: Duration) -> Self {
        let (trigger, receiver) = mpsc::channel(1);
        let trigger_clone = trigger.clone();
        tokio::spawn(async move {
            sleep(duration).await;
            let _ = trigger_clone.send(None).await; // Ignore errors if receiver is dropped
        });
        TimeBomb(trigger, receiver)
    }
}

pub struct Querier {
    cache: Cache,
    tracker: Arc<DashMap<Query, mpsc::Sender<Option<Response>>>>,
}

impl Querier {
    pub fn new(cache: Cache) -> Self {
        Querier {
            cache,
            tracker: Arc::new(DashMap::new()),
        }
    }

    // get tracker
    pub fn tracker(&self) -> Arc<DashMap<Query, mpsc::Sender<Option<Response>>>> {
        Arc::clone(&self.tracker)
    }

    fn prepare_query(&self, query: &Query) -> ChannelMessage {
        // Prepare the query message to be sent over the network
        ChannelMessage {
            ip: "".parse().unwrap(), // Replace with actual IP address
            bytes: Vec::new(), // Assuming Query has a method to convert to bytes
        }
    }

    pub async fn query(&self, query: Query, duration: Duration, listener: &Listener) -> Vec<Arc<Response>> {
        let response = self.cache.get(&query).await;
        if response.is_empty() && !self.tracker.contains_key(&query) {
            // If the response is not cached and not being tracked, we need to send a query
            let query_message = self.prepare_query(&query);
            let TimeBomb(trigger, mut receiver) = TimeBomb::new(duration);
            self.tracker.insert(query.clone(), trigger);

            //  trigger a network query
            if let Err(e) = listener.send(query_message).await {
                eprintln!("Failed to send query: {}", e);
                return vec![];
            }
            
            // Wait for the time bomb to trigger or for a response to be cached
            while let Some(response) = receiver.recv().await {
                if let Some(resp) = response {
                    println!("Querier received response: {:?}", resp);
                    return vec![Arc::new(resp)];
                } else {
                    println!("Querier received timeout, no response found.");
                    // remove the tracker entry
                    self.tracker.remove(&query);
                    break;
                }
            }
            vec![]
        } else {
            response
        }
    }
}
