use serde::{Serialize, Deserialize};
use celestia_integration::poster::{create_namespace, CelestiaClient};
#[derive(Serialize)]
struct RollupBlock {
    transactions: Vec<String>,
    state_root: String,
    block_number: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create namespace for your rollup
    let namespace = create_namespace("my-rollup-v1");
    
    // Initialize Celestia client
    let client = CelestiaClient::new(
        "http://localhost:26658".to_string(),
        namespace,
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o".to_string(),
    );

    // Create example block data
    let block = RollupBlock {
        transactions: vec!["tx1".to_string(), "tx2".to_string()],
        state_root: "0x123...".to_string(),
        block_number: 1,
    };

    // Serialize block to bytes
    let block_data = serde_json::to_vec(&block)?;

    // Submit to Celestia and get span
    match client.submit_pfb(block_data).await {
        Ok(span) => {
            println!("Block submitted successfully!");
            println!("Celestia height: {}", span.height);
            println!("Start share index: {}", span.start_index);
            println!("Number of shares: {}", span.data_len);
        }
        Err(e) => {
            eprintln!("Failed to submit block: {}", e);
        }
    }

    Ok(())
}