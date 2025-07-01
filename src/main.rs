use home_web::*;
use std::{
    collections::HashMap,
    io::{self, BufRead},
    time::Duration,
};

#[tokio::main]
async fn main() {
    let mut hw = HomeWeb::new().expect("Failed to create HomeWeb instance");
    let mut metadata = HashMap::new();
    let ins_name = format!("{}._homecast._tcp.local", random_alphanumeric_string(6));

    metadata.insert("magic".to_string(), random_alphanumeric_string(5));
    _ = hw.register_device(
        Instance::new(ins_name.clone(), 8080, metadata).expect("Failed to create instance"),
    );

    println!("HomeWeb is running... @ {}", ins_name);
    println!(
        "Press Enter to resolve '_homecast._tcp.local' or type a custom name for resolving instance:"
    );
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line.expect("Failed to read line");
        if input.trim().is_empty() {
            // Resolve the service only such as homecast
            let devices = hw
                .get_devices("_homecast._tcp.local".to_string(), Duration::from_secs(3))
                .await;
            println!("Discovered devices: {:?}", devices);
        } else {
            // Resolve the custom name
            let device = hw
                .resolve_device(input.trim().to_string(), Duration::from_secs(3))
                .await;
            println!("Resolved device: {:?}", device);
        }
    }
    println!("Shutting down HomeWeb...");
}
