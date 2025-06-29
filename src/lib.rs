use std::time::{Duration, SystemTime};

use simple_dns::{rdata::RData, ResourceRecord};
use types::*;

mod api;
mod cache;
mod listener;
mod querier;
mod register;
mod responder;
mod types;

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
                Response{
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
                Response{
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
                Response{
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
                Response{
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
                Response{
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