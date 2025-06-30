use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;
use bazuka::{SkmvCache, SkmvConfig};
use dashmap::DashMap;
use simple_dns::Name; use crate::cache::Tracker;

// Import Name type
use super::listener::Listener;
use super::cache::Cache;
use super::register::Registry;
use super::querier::Querier;
use super::responder::Responder;
use super::types::*;

/// HomeWeb API for managing devices in a home network via service discovery.
pub struct HomeWeb {
    register: Registry,
    listener: Arc<Listener>,
    querier: Arc<Querier>,
}

impl HomeWeb {
    async fn resolve_srv(&self, instance: String, duration: Duration) -> Option<(u16, String)> {
        let query = Query {
            qname: Name::new_unchecked(&instance).into_owned(),
            qtype: QueryType::SRV,
        };
        // pick the first response
        self.querier.query(query, duration, false, &self.listener).await.into_iter().next().and_then(|response| {
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
        self.querier.query(query, duration, false, &self.listener).await.iter().for_each(|response| {
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
        // pick the first response
        self.querier.query(query, duration, false, &self.listener).await.into_iter().next().and_then(|response| {
            if let ResponseInner::A { address } = &response.inner {
                Some(vec![*address])
            } else {
                None
            }
        })
    }

    async fn resolve_aaaa(&self, hostname: String, duration: Duration) -> Option<Vec<Ipv6Addr>> {
        let query = Query {
            qname: Name::new_unchecked(&hostname).into_owned(),
            qtype: QueryType::AAAA,
        };
        // pick the first response
        self.querier.query(query, duration, false, &self.listener).await.into_iter().next().and_then(|response| {
            if let ResponseInner::AAAA { address } = &response.inner {
                Some(vec![*address])
            } else {
                None
            }
        })
    }
}

impl HomeWeb {
    pub fn new() -> Result<Self, String> {
        let registry = Registry::new();
        let responder = Responder::new(registry.clone());
        let tracker:Tracker = Arc::new(DashMap::new());
        let listener = Listener::new(tracker.clone(),responder)?;

        let cache:Cache = Arc::new(SkmvCache::new(SkmvConfig{
            idle_timeout: Some(40),
            maximum_capacity:200,
            maximum_values_per_key:2,
            time_to_live: Some(120)
        }));

        let querier = Querier::new(cache.clone(), tracker.clone(), listener.clone());

        Ok(HomeWeb {
            register: registry,
            querier,
            listener,
        })
    }

    pub async fn get_devices(&self, svc_type: String, duration: Duration) -> Vec<String> {
        let query = Query{
            qname: Name::new_unchecked(&svc_type).into_owned(),
            qtype: QueryType::PTR
        };
        self.querier.query(query, duration, false, &self.listener).await.iter()
            .filter_map(|response| {
                if let ResponseInner::PTR(ptr) = &response.inner {
                    Some(ptr.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    pub async fn resolve_device(&self, instance_name: String, duration: Duration) -> Option<Device> {
        let (port, target) = self.resolve_srv(instance_name.clone(), duration).await?;
        let txt = self.resolve_txt(instance_name.clone(), duration).await;
        let a_records = self.resolve_a(target.clone(), duration).await?.into_iter().map(std::net::IpAddr::V4).collect::<Vec<_>>();
        let aaaa_records = self.resolve_aaaa(target.clone(), duration).await?.into_iter().map(std::net::IpAddr::V6).collect::<Vec<_>>();
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
