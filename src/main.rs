use serde::{Serialize, Deserialize};
use celestia_integration::poster::CelestiaClient;

#[derive(Debug, Serialize)]
struct RollupBlock {
    transactions: Vec<String>,
    state_root: String,
    block_number: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Celestia client
    let client = CelestiaClient::new().await?;

    // Create example block data
    let block = RollupBlock {
        transactions: vec!["tx1".to_string(), "tx2".to_string()],
        state_root: "0x123...".to_string(),
        block_number: 1,
    };

    println!("Submitting rollup block: {:?}", block);

    // Serialize block to bytes
    let block_data = serde_json::to_vec(&block)?;

    // Submit to Celestia and get span
    match client.submit_pfb(block_data).await {
        Ok(span) => {
            println!("Block submitted successfully!");
            println!("Celestia height: {}", span.height);
            println!("Start share index: {}", span.start_index);
            println!("Number of shares: {}", span.data_len);

            // Verify we can retrieve the data
            println!("\nVerifying data retrieval...");
            let namespace_data = client.get_shares_by_height(span.height).await?;
            if !namespace_data.rows.is_empty() {
                println!("Successfully retrieved namespace data");
                println!("Number of rows: {}", namespace_data.rows.len());
            } else {
                println!("No data found in namespace");
            }
        }
        Err(e) => {
            eprintln!("Failed to submit block: {}", e);
        }
    }

    Ok(())
}