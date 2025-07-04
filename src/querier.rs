use super::cache::*;
use super::listener::Listener;
use super::types::*;
use simple_dns::{CLASS, Packet, QCLASS, Question, ResourceRecord};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tokio::time::sleep;

struct TimeBomb(
    mpsc::Sender<Option<(Query, Response, u32)>>,
    mpsc::Receiver<Option<(Query, Response, u32)>>,
);

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
    tracker: Tracker,
}

impl Querier {
    fn should_refresh(remaining_secs: u64, lifetime_secs: u32) -> bool {
        use rand::{Rng, rng};
        let threshold_percent = rng().random_range(85..=95) as f64 / 100.0;
        let threshold_secs = (lifetime_secs as f64 * threshold_percent).round() as u64;
        print!(
            "@ Remaining: {}, Threshold: {} ",
            remaining_secs, threshold_secs
        );
        remaining_secs <= threshold_secs
    }
}

impl Querier {
    pub fn new(cache: Cache, tracker: Tracker, listener: Arc<Listener>) -> Arc<Self> {
        let querier = Arc::new(Querier { cache, tracker });
        let querier_clone = querier.clone();
        tokio::spawn(async move {
            //refesh every 60 seconds
            loop {
                sleep(Duration::from_secs(60)).await;
                querier_clone.refresh_cache(&listener).await;
            }
        });
        querier
    }

    async fn refresh_cache(&self, listener: &Listener) {
        // iterate the entire cache and if the end time is about to expire lets say 10 seconds left only then will execute the query
        println!("Refreshing cache...");
        let mut queries_to_refresh = Vec::new();
        let now = SystemTime::now();
        for (query, response, ttl) in self.cache.iter().await {
            let remaining_ttl = response
                .ends_at
                .duration_since(now)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();
            if Querier::should_refresh(remaining_ttl, ttl) {
                println!("Refreshing {:?}", query);
                queries_to_refresh.push((*query).clone());
            }
        }

        for query in queries_to_refresh {
            let _ = self
                .query(query, Duration::from_secs(5), true, listener)
                .await;
        }
    }

    async fn prepare_query(&self, query: &Query) -> Option<Vec<u8>> {
        // make a query packet
        let mut packet = Packet::new_query(0);
        packet.questions.push(Question::new(
            query.qname.clone(),
            query.qtype.clone().into(),
            QCLASS::CLASS(CLASS::IN),
            false,
        ));

        // add previous known answers to the packet for answer supression
        let responses = self.cache.get(query).await;
        if !responses.is_empty() {
            for response in responses {
                let remaining_ttl = response
                    .ends_at
                    .duration_since(SystemTime::now())
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs() as u32;
                let record = ResourceRecord::new(
                    query.qname.clone(),
                    CLASS::IN,
                    remaining_ttl,
                    response.inner.clone().into(),
                );
                packet.answers.push(record);
            }
        }
        // Serialize the packet to bytes
        return super::serialize_packet(&mut packet);
    }

    pub async fn query(
        &self,
        query: Query,
        duration: Duration,
        bypass_cache: bool,
        listener: &Listener,
    ) -> Vec<Arc<Response>> {
        let response = self.cache.get(&query).await;
        if bypass_cache || (response.is_empty() && !self.tracker.contains_key(&query)) {
            // If the response is not cached and not being tracked, we need to send a query
            let query_message = self.prepare_query(&query).await;
            let TimeBomb(trigger, mut receiver) = TimeBomb::new(duration);
            self.tracker.insert(query.clone(), trigger);
            let query_message = match query_message {
                Some(msg) => msg,
                None => {
                    eprintln!("Failed to prepare query message.");
                    self.tracker.remove(&query);
                    return vec![];
                }
            };
            // trigger a network query
            if let Err(e) = listener
                .send(ChannelMessage {
                    ip: *super::multicast_addr_v4(),
                    bytes: query_message.clone(),
                })
                .await
            {
                eprintln!("Failed to send query in ip4 socket: {}", e);
                // return vec![];
            }

            if let Err(e) = listener
                .send(ChannelMessage {
                    ip: *super::multicast_addr_v6(),
                    bytes: query_message,
                })
                .await
            {
                eprintln!("Failed to send query in ip6 socket: {}", e);
                // return vec![];
            }

            // Wait for the time bomb to trigger or for a response to be cached
            while let Some(response) = receiver.recv().await {
                if let Some((qry, response, ttl)) = response {
                    self.cache.insert(qry, response, ttl).await;
                } else {
                    let cache_resp = self.cache.get(&query).await;
                    println!(
                        "Querier received timeout, for {:?} with {:?}",
                        query, cache_resp
                    );
                    self.tracker.remove(&query);
                    return cache_resp;
                }
            }
            vec![]
        } else {
            response
        }
    }
}
