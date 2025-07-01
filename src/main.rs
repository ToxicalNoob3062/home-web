use std::{collections::HashMap, time::Duration};

use home_web::*;

#[tokio::main]
async fn main() {
    let hw = HomeWeb::new().expect("Failed to create HomeWeb instance");
    // let mut metadata = HashMap::new();
    // // add a metadata entry for the service type
    // metadata.insert("service_type".to_string(), "homecast".to_string());
    // _ = hw.register_device(Instance::new(
    //     "linux._homecast._tcp.local".to_string(),
    //     8080,
    //     metadata,
    // ).expect("Failed to create instance"));

    // println!("HomeWeb is running...");

    // whever i press q trigger query
    let device = hw
        .resolve_device(
            "potato._homecast._tcp.local".to_string(),
            Duration::from_secs(5),
        )
        .await
        .expect("Failed to resolve device");
    println!("Resolved device: {:?}", device);

    // // wait until i press control c
    // tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
    // println!("Shutting down HomeWeb...");
}
