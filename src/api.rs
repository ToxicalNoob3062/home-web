use bazuka::{SkmvCache, SkmvConfig};
use dashmap::DashMap;
use simple_dns::Name;
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;

// Import Name type
use super::cache::Cache;
use super::cache::Tracker;
use super::listener::Listener;
use super::querier::Querier;
use super::register::Registry;
use super::responder::Responder;
use super::types::*;

/// HomeWeb API for managing devices in a home network via service discovery.
pub struct HomeWeb {
    register: Registry,
    listener: Arc<Listener>,
    querier: Arc<Querier>,
    cache: Cache,
}

impl HomeWeb {
    async fn resolve_srv(&self, instance: String, duration: Duration) -> Option<(u16, String)> {
        let query = Query {
            qname: Name::new_unchecked(&instance).into_owned(),
            qtype: QueryType::SRV,
        };
        // pick the first response
        self.querier
            .query(query, duration, false, &self.listener)
            .await
            .into_iter()
            .next()
            .and_then(|response| {
                if let ResponseInner::SRV { port, target } = &response.inner {
                    Some((*port, target.clone()))
                } else {
                    None
                }
            })
    }
    async fn resolve_txt(&self, instance: String, duration: Duration) -> HashMap<String, String> {
        let query = Query {
            qname: Name::new_unchecked(&instance).into_owned(),
            qtype: QueryType::TXT,
        };
        // pick the first response
        let mut map = HashMap::new();
        self.querier
            .query(query, duration, false, &self.listener)
            .await
            .iter()
            .for_each(|response| {
                if let ResponseInner::TXT { strings } = &response.inner {
                    for string in strings {
                        let parts: Vec<&str> = string.split('=').collect();
                        if parts.len() == 2 {
                            map.insert(parts[0].to_string(), parts[1].to_string());
                        }
                    }
                }
            });
        map
    }

    async fn resolve_a(&self, hostname: String, duration: Duration) -> Option<Vec<Ipv4Addr>> {
        let query = Query {
            qname: Name::new_unchecked(&hostname).into_owned(),
            qtype: QueryType::A,
        };

        let responses = self
            .querier
            .query(query, duration, false, &self.listener)
            .await;

        let addresses = responses
            .into_iter()
            .filter_map(|response| {
                if let ResponseInner::A { address } = response.inner {
                    Some(address)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if addresses.is_empty() {
            None
        } else {
            Some(addresses)
        }
    }

    async fn resolve_aaaa(&self, hostname: String, duration: Duration) -> Option<Vec<Ipv6Addr>> {
        let query = Query {
            qname: Name::new_unchecked(&hostname).into_owned(),
            qtype: QueryType::AAAA,
        };

        let responses = self
            .querier
            .query(query, duration, false, &self.listener)
            .await;

        let addresses = responses
            .into_iter()
            .filter_map(|response| {
                if let ResponseInner::AAAA { address } = response.inner {
                    Some(address)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if addresses.is_empty() {
            None
        } else {
            Some(addresses)
        }
    }
}

impl HomeWeb {
    pub fn new() -> Result<Self, String> {
        let registry = Registry::new();
        let responder = Responder::new(registry.clone());
        let tracker: Tracker = Arc::new(DashMap::new());
        let listener = Listener::new(tracker.clone(), responder)?;

        let cache: Cache = Arc::new(SkmvCache::new(SkmvConfig {
            idle_timeout: Some(40),
            maximum_capacity: 200,
            maximum_values_per_key: 2,
            time_to_live: Some(120),
        }));

        let querier = Querier::new(cache.clone(), tracker.clone(), listener.clone());

        Ok(HomeWeb {
            register: registry,
            querier,
            listener,
            cache,
        })
    }

    pub async fn get_devices(&self, svc_type: String, duration: Duration) -> Vec<String> {
        let query = Query {
            qname: Name::new_unchecked(&svc_type).into_owned(),
            qtype: QueryType::PTR,
        };
        let responses = self
            .querier
            .query(query, duration, false, &self.listener)
            .await
            .iter()
            .filter_map(|response| {
                if let ResponseInner::PTR(ptr) = &response.inner {
                    Some(ptr.clone())
                } else {
                    None
                }
            })
            .collect();
        //  print cache
        println!("Current cache: {:#?}", self.cache);
        responses
    }
    pub async fn resolve_device(
        &self,
        instance_name: String,
        duration: Duration,
    ) -> Option<Device> {
        // Split duration: 20% SRV, 30% TXT, 25% A, 25% AAAA
        let dur_srv = duration.mul_f32(0.20);
        let dur_txt = duration.mul_f32(0.30);
        let dur_a = duration.mul_f32(0.25);
        let dur_aaaa = duration.mul_f32(0.25);

        // Step 1: Resolve SRV
        let (port, target) = self.resolve_srv(instance_name.clone(), dur_srv).await?;

        // Step 2: Resolve TXT
        let txt = self.resolve_txt(instance_name.clone(), dur_txt).await;

        // Step 3: Resolve A
        let a_records = self
            .resolve_a(target.clone(), dur_a)
            .await?
            .into_iter()
            .map(std::net::IpAddr::V4)
            .collect::<Vec<_>>();

        // Step 4: Resolve AAAA
        let aaaa_records = self
            .resolve_aaaa(target.clone(), dur_aaaa)
            .await?
            .into_iter()
            .map(std::net::IpAddr::V6)
            .collect::<Vec<_>>();

        // Debug: Print current cache
        println!("Current cache: {:#?}", self.cache);

        // Build and return Device
        Some(Device {
            name: instance_name,
            port,
            host: target,
            metadata: txt,
            addresses: [a_records, aaaa_records].concat(),
        })
    }

    pub fn register_device(&mut self, instance: Instance) -> Result<(), String> {
        self.register.register_device(instance);
        Ok(())
    }
    pub fn unregister_device(&mut self, instance: &Instance) -> Result<(), String> {
        self.register.unregister_device(instance);
        Ok(())
    }
}
