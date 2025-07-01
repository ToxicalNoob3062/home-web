use super::register::Registry;
use simple_dns::{
    CLASS, Name, Packet, QTYPE, Question, ResourceRecord, TYPE,
    rdata::{A, AAAA, PTR, RData, SRV},
};
#[derive(Debug)]
pub struct Responder {
    registry: Registry,
}

impl Responder {
    pub fn new(registry: Registry) -> Self {
        Responder { registry }
    }

    fn inject_ptr_records<'a>(
        &self,
        qname: &Name<'a>,
        packet: &mut Packet<'a>,
    ) -> Result<(), String> {
        let service_type = qname.to_string();
        let instance_names = self.registry.get_instance_names(&service_type)?;
        instance_names.iter().for_each(|instance_name| {
            let record = ResourceRecord::new(
                qname.clone(),
                CLASS::IN,
                120,
                RData::PTR(PTR(Name::new_unchecked(instance_name).into_owned())),
            );
            packet.answers.push(record);
        });
        Ok(())
    }

    fn inject_srv_records<'a>(
        &self,
        ascope: bool,
        qname: &Name<'a>,
        packet: &mut Packet<'a>,
    ) -> Result<(), String> {
        let instance = self.registry.get_instance(&qname.to_string())?;
        let record = ResourceRecord::new(
            qname.clone(),
            CLASS::IN,
            120,
            RData::SRV(SRV {
                priority: 0,
                weight: 0,
                port: instance.port(),
                target: Name::new_unchecked(super::mdns_hostname()).into_owned(),
            }),
        );
        if ascope {
            packet.answers.push(record);
        } else {
            packet.additional_records.push(record);
        }
        Ok(())
    }

    fn inject_txt_records<'a>(
        &self,
        ascope: bool,
        qname: &Name<'a>,
        packet: &mut Packet<'a>,
    ) -> Result<(), String> {
        let instance = self.registry.get_instance(&qname.to_string())?;
        let metadata: Vec<String> = instance
            .metadata()
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        // If metadata is empty, we do not inject a TXT record
        if !metadata.is_empty() {
            let txt_record = ResourceRecord::new(
                qname.clone(),
                CLASS::IN,
                120,
                RData::TXT(super::form_text_record(metadata.as_ref())),
            );
            if ascope {
                packet.answers.push(txt_record);
            } else {
                packet.additional_records.push(txt_record);
            }
            return Ok(());
        }
        Err(format!("Instance not found for query: {}", qname))
    }

    // Injects A records into the provided packet.
    fn inject_a_records<'a>(&self, ascope: bool, packet: &mut Packet<'a>) {
        for ip in Registry::get_ip4_list() {
            let record = ResourceRecord::new(
                Name::new_unchecked(super::mdns_hostname()).into_owned(),
                CLASS::IN,
                120,
                RData::A(A { address: ip.into() }),
            );
            if ascope {
                packet.answers.push(record);
            } else {
                packet.additional_records.push(record);
            }
        }
    }

    // Injects AAAA records into the provided packet.
    fn inject_aaaa_records<'a>(&self, ascope: bool, packet: &mut Packet<'a>) {
        for ip6 in Registry::get_ip6_list() {
            let record = ResourceRecord::new(
                Name::new_unchecked(super::mdns_hostname()).into_owned(),
                CLASS::IN,
                120,
                RData::AAAA(AAAA {
                    address: ip6.into(),
                }),
            );
            if ascope {
                packet.answers.push(record);
            } else {
                packet.additional_records.push(record);
            }
        }
    }

    // Prepares a response packet for PTR queries by injecting PTR, SRV, and TXT records.
    fn prepare_ptr_response<'a>(
        &self,
        qname: &Name<'a>,
        response_packet: &mut Packet<'a>,
    ) -> Result<(), String> {
        self.inject_ptr_records(qname, response_packet)?;
        let ptr_records: Vec<_> = response_packet
            .answers
            .iter()
            .filter_map(|record| {
                if let RData::PTR(ptr) = &record.rdata {
                    Some(ptr.clone())
                } else {
                    None
                }
            })
            .collect();
        for ptr in ptr_records {
            self.inject_srv_records(false, &ptr, response_packet)?;
            self.inject_txt_records(false, &ptr, response_packet)?;
        }
        Ok(())
    }

    // Prepares a response packet for SRV queries by injecting SRV, A, and AAAA records.
    fn prepare_srv_response<'a>(
        &self,
        qname: &Name<'a>,
        response_packet: &mut Packet<'a>,
    ) -> Result<(), String> {
        self.inject_srv_records(true, qname, response_packet)?;
        if let Some(first_srv) = response_packet.answers.first() {
            if let RData::SRV(_) = &first_srv.rdata {
                self.inject_a_records(false, response_packet);
                self.inject_aaaa_records(false, response_packet);
            }
        }
        Ok(())
    }

    pub fn answer_queries<'a>(&self, questions: Vec<Question<'a>>) -> Packet<'a> {
        let mut response_packet = Packet::new_reply(0);
        for question in questions {
            if let QTYPE::TYPE(qtype) = question.qtype {
                match qtype {
                    TYPE::PTR => {
                        _ = self.prepare_ptr_response(&question.qname, &mut response_packet);
                    }
                    TYPE::SRV => {
                        _ = self.prepare_srv_response(&question.qname, &mut response_packet);
                    }
                    TYPE::TXT => {
                        _ = self.inject_txt_records(true, &question.qname, &mut response_packet);
                    }
                    TYPE::A => {
                        self.inject_a_records(true, &mut response_packet);
                    }
                    TYPE::AAAA => {
                        self.inject_aaaa_records(true, &mut response_packet);
                    }
                    _ => {}
                }
            }
        }
        response_packet
    }

    pub fn suppress_known_answers<'a>(
        prepared_answers: &mut Vec<ResourceRecord<'a>>,
        known_answers: &[ResourceRecord<'a>],
    ) {
        prepared_answers.retain(|r| {
            if let Some(triplet) = super::prepare_triplet_from_record(r) {
                !known_answers.iter().any(|ka| {
                    if let Some(ka_triplet) = super::prepare_triplet_from_record(ka) {
                        triplet.0 == ka_triplet.0
                            && triplet.1 == ka_triplet.1
                            && ka_triplet.2 >= triplet.2 / 2
                    } else {
                        false
                    }
                })
            } else {
                true
            }
        });
    }
}
