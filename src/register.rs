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
