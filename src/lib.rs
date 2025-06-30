use simple_dns::{
    Packet, ResourceRecord,
    rdata::{RData, TXT},
};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};
use types::*;

mod api;
mod cache;
mod listener;
mod querier;
mod register;
mod responder;
mod types;

#[macro_export]
macro_rules! global {
    ($static_name:ident, $fn_name:ident, $type:ty, $init:expr) => {
        static $static_name: OnceLock<$type> = OnceLock::new();
        pub fn $fn_name() -> &'static $type {
            $static_name.get_or_init(|| $init)
        }
    };
}

// Defining some global varibales which wont change during the runtime
global!(
    MULTICAST_ADDR_V4,
    multicast_addr_v4,
    SocketAddr,
    "224.0.0.251:5353".parse().unwrap()
);
global!(
    MULTICAST_ADDR_V6,
    multicast_addr_v6,
    SocketAddr,
    "[ff02::fb]:5353".parse().unwrap()
);

pub fn prepare_triplet_from_record<'a>(
    record: &ResourceRecord<'a>,
) -> Option<(Query, Response, u32)> {
    let mut triplet: Option<(Query, Response, u32)> = None;
    match &record.rdata {
        RData::PTR(ptr) => {
            triplet = Some((
                Query {
                    qname: record.name.clone().into_owned(),
                    qtype: QueryType::PTR,
                },
                Response {
                    inner: ResponseInner::PTR(ptr.to_string()),
                    ends_at: SystemTime::now() + Duration::from_secs(record.ttl as u64),
                },
                record.ttl,
            ));
        }
        RData::SRV(srv) => {
            triplet = Some((
                Query {
                    qname: record.name.clone().into_owned(),
                    qtype: QueryType::SRV,
                },
                Response {
                    inner: ResponseInner::SRV {
                        port: srv.port,
                        target: srv.target.to_string(),
                    },
                    ends_at: SystemTime::now() + Duration::from_secs(record.ttl as u64),
                },
                record.ttl,
            ));
        }
        RData::TXT(txt) => {
            triplet = Some((
                Query {
                    qname: record.name.clone().into_owned(),
                    qtype: QueryType::TXT,
                },
                Response {
                    inner: ResponseInner::TXT {
                        strings: txt
                            .attributes()
                            .into_iter()
                            .filter_map(|(k, v)| v.map(|val| format!("{}={}", k, val)))
                            .collect(),
                    },
                    ends_at: SystemTime::now() + Duration::from_secs(record.ttl as u64),
                },
                record.ttl,
            ));
        }
        RData::A(a) => {
            triplet = Some((
                Query {
                    qname: record.name.clone().into_owned(),
                    qtype: QueryType::A,
                },
                Response {
                    inner: ResponseInner::A {
                        address: a.address.into(),
                    },
                    ends_at: SystemTime::now() + Duration::from_secs(record.ttl as u64),
                },
                record.ttl,
            ));
        }
        RData::AAAA(aaaa) => {
            triplet = Some((
                Query {
                    qname: record.name.clone().into_owned(),
                    qtype: QueryType::AAAA,
                },
                Response {
                    inner: ResponseInner::AAAA {
                        address: aaaa.address.into(),
                    },
                    ends_at: SystemTime::now() + Duration::from_secs(record.ttl as u64),
                },
                record.ttl,
            ));
        }
        _ => {}
    }
    triplet
}

fn form_text_record(metadata: &Vec<String>) -> TXT<'static> {
    let mut text_data = TXT::new();
    metadata.iter().for_each(|pair_string| {
        _ = text_data.add_string(pair_string);
    });
    text_data.into_owned()
}

fn reduce_packet_size(packet: &mut Packet, max_size: usize) -> bool {
    let mut bytes = Vec::new();
    while packet.write_to(&mut bytes).is_err() || bytes.len() > max_size {
        if !packet.additional_records.is_empty() {
            packet.additional_records.pop();
        } else if !packet.answers.is_empty() {
            packet.answers.pop();
        } else {
            return false;
        }
        bytes.clear();
    }
    true
}
