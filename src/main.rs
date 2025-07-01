use std::{collections::HashMap, time::Duration};

use home_web::*;

#[tokio::main]
async fn main() {
    let mut hw = HomeWeb::new().expect("Failed to create HomeWeb instance");
    let mut metadata = HashMap::new();

    // add a metadata entry for the service type
    metadata.insert("magic".to_string(), random_alphanumeric_string(5));
    _ = hw.register_device(
        Instance::new("linux._homecast._tcp.local".to_string(), 8080, metadata)
            .expect("Failed to create instance"),
    );

    println!("HomeWeb is running...");
    // whever i press ctrl+c it will resolve the device
    while tokio::signal::ctrl_c().await.is_ok() {
        let device = hw
            .resolve_device(
                "potato._homecast._tcp.local".to_string(),
                Duration::from_secs(5),
            )
            .await;
        println!("Resolved device: {:?}", device);
    }
    println!("Shutting down HomeWeb...");
}
