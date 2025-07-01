use std::{collections::HashMap, io::{self, BufRead}, time::Duration};
use home_web::*;

#[tokio::main]
async fn main() {
    let mut hw = HomeWeb::new().expect("Failed to create HomeWeb instance");
    let mut metadata = HashMap::new();

    metadata.insert("magic".to_string(), random_alphanumeric_string(5));
    _ = hw.register_device(
        Instance::new("linux._homecast._tcp.local".to_string(), 8080, metadata)
            .expect("Failed to create instance"),
    );

    println!("HomeWeb is running...");
    println!("Press Enter to resolve 'potato._homecast._tcp.local' or type a custom name:");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let query_name = match line {
            Ok(ref l) if !l.trim().is_empty() => l.trim().to_string(),
            _ => "potato._homecast._tcp.local".to_string(),
        };

        let device = hw
            .resolve_device(query_name, Duration::from_secs(5))
            .await;
        println!("Resolved device: {:?}", device);
        println!("\nType another query name or press Enter again:");
    }

    println!("Shutting down HomeWeb...");
}

