use std::{collections::HashMap, net::{IpAddr, Ipv4Addr, Ipv6Addr}};

use dashmap::{DashMap, DashSet};

use super::types::Instance;

pub struct Registry {
    devices: DashMap<String, DashSet<Instance>>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            devices: DashMap::new(),
        }
    }

    pub fn get_instance_names(&self, stype: &str) -> Result<Vec<String>, String> {
        if let Some(instances) = self.devices.get(stype) {
            Ok(instances.iter().map(|i| i.name().to_string()).collect())
        } else {
            Err(format!("No instances found for service type: {}", stype))
        }
    }

    pub fn get_instance(&self, instance: &str) -> Result<Instance, String> {
        let service_type = Instance::break_instance_str(instance)?;
        if let Some(instances) = self.devices.get(&service_type) {
            if let Some(instance) =
                instances.get(&Instance::new(instance.to_string(), 0, HashMap::new())?)
            {
                return Ok(instance.clone());
            }
        }
        Err(format!("Instance not found: {}", instance))
    }

    pub fn get_ip4_list() -> Vec<Ipv4Addr> {
        let mut ip4_list = Vec::new();
        let interfaces_x = local_ip_address::list_afinet_netifas();
        if let Ok(interfaces) = interfaces_x {
            for interface in interfaces {
                match interface.1 {
                    IpAddr::V4(v4) => {
                        ip4_list.push(v4);
                    }
                    _ => {}
                }
            }
        }
        println!("IP4 List: {:?}", ip4_list);
        ip4_list
    }

    pub fn get_ip6_list() -> Vec<Ipv6Addr> {
        let mut ip6_list = Vec::new();
        let interfaces_x = local_ip_address::list_afinet_netifas();
        if let Ok(interfaces) = interfaces_x {
            for interface in interfaces {
                match interface.1 {
                    IpAddr::V6(v6) => {
                        ip6_list.push(v6);
                    }
                    _ => {}
                }
            }
        }
        println!("IP6 List: {:?}", ip6_list);
        ip6_list
    }

    pub fn register_device(&mut self, instance: Instance) {
        let service_type = instance.service_type();
        let instances = self.devices.entry(service_type).or_default();
        instances.insert(instance);
    }

    pub fn unregister_device(&mut self, instance: &Instance) {
        if let Some(instances) = self.devices.get_mut(&instance.service_type()) {
            instances.remove(instance);
            if instances.is_empty() {
                self.devices.remove(&instance.service_type());
            }
        }
    }
}
