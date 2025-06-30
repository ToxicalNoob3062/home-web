use super::cache::*;
use super::listener::Listener;
use super::types::*;
use simple_dns::{CLASS, Packet, QCLASS, Question, ResourceRecord};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tokio::time::sleep;

struct TimeBomb(
    mpsc::Sender<Option<(Response, u32)>>,
    mpsc::Receiver<Option<(Response, u32)>>,
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
    pub fn new(cache: Cache, tracker: Tracker) -> Self {
        Querier { cache, tracker }
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
        // packet reduction
        if !super::reduce_packet_size(&mut packet, 1472) {
            return None;
        }

        // Convert the packet to bytes
        let mut response_bytes = Vec::new();
        if packet.write_to(&mut response_bytes).is_ok() {
            Some(response_bytes)
        } else {
            None
        }
    }

    pub async fn query(
        &mut self,
        query: Query,
        duration: Duration,
        listener: &Listener,
    ) -> Vec<Arc<Response>> {
        let response = self.cache.get(&query).await;
        if response.is_empty() && !self.tracker.contains_key(&query) {
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
                eprintln!("Failed to send query: {}", e);
                return vec![];
            }

            if let Err(e) = listener
                .send(ChannelMessage {
                    ip: *super::multicast_addr_v6(),
                    bytes: query_message,
                })
                .await
            {
                eprintln!("Failed to send query: {}", e);
                return vec![];
            }

            // Wait for the time bomb to trigger or for a response to be cached
            while let Some(response) = receiver.recv().await {
                if let Some(resp) = response {
                    println!("Querier received response: {:?}", resp);
                    self.cache.insert(query.clone(), resp.0, resp.1).await;
                } else {
                    println!("Querier received timeout, no response found.");
                    self.tracker.remove(&query);
                    return self.cache.get(&query).await;
                }
            }
            vec![]
        } else {
            response
        }
    }
}
