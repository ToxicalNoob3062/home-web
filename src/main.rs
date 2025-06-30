use std::collections::HashMap;

use home_web::*;


#[tokio::main]
async fn main() {
    let mut hw = HomeWeb::new().expect("Failed to create HomeWeb instance");
    let mut metadata = HashMap::new();
    // add a metadata entry for the service type
    metadata.insert("service_type".to_string(), "homecast".to_string());
    _ = hw.register_device(Instance::new(
        "linux._homecast._tcp.local".to_string(),
        8080,
        metadata,
    ).expect("Failed to create instance"));

    println!("HomeWeb is running...");
    // wait until i press control c
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
    println!("Shutting down HomeWeb...");
}
