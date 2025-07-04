use simple_dns::{Name, QTYPE, TYPE, rdata::*};
use std::time::SystemTime;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

#[derive(Debug)]
pub struct Device {
    pub name: String,
    pub port: u16,
    pub host: String,
    pub addresses: Vec<IpAddr>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Instance {
    name: String,
    port: u16,
    metadata: HashMap<String, String>,
}

impl Instance {
    // constructor for Instance
    pub fn new(name: String, port: u16, metadata: HashMap<String, String>) -> Result<Self, String> {
        Self::validate(&name, port)?;
        Ok(Instance {
            name,
            port,
            metadata,
        })
    }
    // getter for service type
    pub fn service_type(&self) -> String {
        self.name[self.name.find('.').unwrap() + 1..].to_string()
    }
    // getters for name, port, and metadata
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    pub fn break_instance_str(instance: &str) -> Result<String, String> {
        let parts: Vec<&str> = instance.split('.').collect();
        if parts.len() < 4 {
            return Err(format!(
                "Instance name must be in the format `name.service_type.protocol.local`."
            ));
        }
        let service_type = format!("{}.{}.{}", parts[1], parts[2], parts[3]);
        Ok(service_type)
    }

    //validate the name format and port number
    fn validate(name: &str, port: u16) -> Result<(), String> {
        if name.is_empty() || !name.contains('.') {
            return Self::is_valid_name(name);
        }
        if port == 0 {
            return Err("Invalid port number".to_string());
        }
        Ok(())
    }

    fn is_valid_name(name: &str) -> Result<(), String> {
        // 1. No spaces
        if name.contains(' ') {
            return Err("Instance name should not contain spaces".to_string());
        }

        // 2. Split into exactly 4 parts
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() != 4 {
            return Err(
                "Instance name should be of format 'name._service-type._protocol._domain'"
                    .to_string(),
            );
        }

        // 3. Instance name: lowercase alphanumeric with hyphens
        let is_valid = |s: &str| {
            s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        };
        if !is_valid(parts[0]) {
            return Err(
                "Instance name should be a lowercase alphanumeric string with hyphens".to_string(),
            );
        }

        // 4. Service type: starts with '_', rest is lowercase alphanumeric with hyphens
        if !parts[1].starts_with('_') {
            return Err("Service type should start with an underscore".to_string());
        }
        let service_name = &parts[1][1..]; // Skip the '_'
        if service_name.is_empty() {
            return Err("Service type should not be empty after the underscore".to_string());
        }
        if !is_valid(service_name) {
            return Err(
                "Service type should be a lowercase alphanumeric string with hyphens".to_string(),
            );
        }

        // 5. Protocol: exactly '_tcp' or '_udp'
        if parts[2] != "_tcp" && parts[2] != "_udp" {
            return Err("Protocol should be either '_tcp' or '_udp'".to_string());
        }

        // 6. Domain: exactly 'local'
        if parts[3] != "local" {
            return Err("Domain should be 'local'".to_string());
        }

        Ok(())
    }
}

// instance will be hashed in hashsets using the value of their name only
impl PartialEq for Instance {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Instance {}

impl std::hash::Hash for Instance {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QueryType {
    PTR,
    SRV,
    TXT,
    A,
    AAAA,
}

// implement into() for QueryType to convert to simple_dns::QType
impl From<QueryType> for QTYPE {
    fn from(qtype: QueryType) -> Self {
        match qtype {
            QueryType::PTR => QTYPE::TYPE(TYPE::PTR),
            QueryType::SRV => QTYPE::TYPE(TYPE::SRV),
            QueryType::TXT => QTYPE::TYPE(TYPE::TXT),
            QueryType::A => QTYPE::TYPE(TYPE::A),
            QueryType::AAAA => QTYPE::TYPE(TYPE::AAAA),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Query {
    pub qname: Name<'static>,
    pub qtype: QueryType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResponseInner {
    PTR(String),
    SRV { port: u16, target: String },
    TXT { strings: Vec<String> },
    A { address: Ipv4Addr },
    AAAA { address: Ipv6Addr },
}

// response inner into simple_dns::RData
impl<'a> From<ResponseInner> for RData<'a> {
    fn from(response: ResponseInner) -> Self {
        match response {
            ResponseInner::PTR(ptr) => RData::PTR(PTR(Name::new_unchecked(&ptr).into_owned())),
            ResponseInner::SRV { port, target } => RData::SRV(SRV {
                priority: 0,
                weight: 0,
                port,
                target: Name::new_unchecked(&target).into_owned(),
            }),
            ResponseInner::TXT { strings } => RData::TXT(super::form_text_record(&strings)),
            ResponseInner::A { address } => RData::A(A {
                address: address.into(),
            }),
            ResponseInner::AAAA { address } => RData::AAAA(AAAA {
                address: address.into(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub inner: ResponseInner,
    pub ends_at: SystemTime,
}

impl Hash for Response {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state); // Only hash inner
    }
}

impl PartialEq for Response {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner // Only compare inner
    }
}
impl Eq for Response {}

#[derive(Debug)]
pub struct ChannelMessage {
    pub ip: SocketAddr,
    pub bytes: Vec<u8>,
}

// write test to see if insert same query response twice duplicated by the cache or not.
#[cfg(test)]
mod tests {
    use super::*;
    use bazuka::*;

    #[tokio::test]
    async fn test_cache_insert_duplicate() {
        let cache = SkmvCache::new(SkmvConfig {
            idle_timeout: Some(40),
            maximum_capacity: 200,
            maximum_values_per_key: 2,
            time_to_live: Some(120),
        });
        let query = Query {
            qname: Name::new_unchecked("example.local"),
            qtype: QueryType::PTR,
        };
        let response = Response {
            inner: ResponseInner::PTR("example.local".to_string()),
            ends_at: SystemTime::now(),
        };

        cache.insert(query.clone(), response.clone(), 5).await;
        cache.insert(query, response, 10).await;
        assert_eq!(cache.iter().await.count(), 1); // Should still be 1
    }
}
