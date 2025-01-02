// use serde::{Serialize, Deserialize};
// use tracing_subscriber;
// pub mod celestia_endpoints;
// pub mod celestia_prover;
// pub mod poster;
// pub mod settlement_verification;
// use poster::CelestiaClient;
// #[derive(Debug, Serialize)]
// struct RollupBlock {
//     transactions: Vec<String>,
//     state_root: String,
//     block_number: u64,
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     // Initialize tracing
//     tracing_subscriber::fmt::init();

//     // Initialize Celestia client
//     let client = CelestiaClient::new().await?;

//     // Create example block data
//     let block = RollupBlock {
//         transactions: vec!["tx1".to_string(), "tx2".to_string()],
//         state_root: "0x123...".to_string(),
//         block_number: 1,
//     };

//     println!("Submitting rollup block: {:?}", block);

//     // Serialize block to bytes
//     let block_data = serde_json::to_vec(&block)?;

//     // Submit to Celestia and get span
//     match client.submit_pfb(block_data).await {
//         Ok(span) => {
//             println!("Block submitted successfully!");
//             println!("Celestia height: {}", span.height);
//             println!("Start share index: {}", span.start_index);
//             println!("Number of shares: {}", span.data_len);

//             // Verify we can retrieve the data
//             println!("\nVerifying data retrieval...");
//             let namespace_data = client.get_shares_by_height(span.height).await?;
//             if !namespace_data.rows.is_empty() {
//                 println!("Successfully retrieved namespace data");
//                 println!("Number of rows: {}", namespace_data.rows.len());
//             } else {
//                 println!("No data found in namespace");
//             }
//         }
//         Err(e) => {
//             eprintln!("Failed to submit block: {}", e);
//         }
//     }

//     Ok(())
// }
mod celestia_prover;
mod celestia_endpoints;
mod poster;
mod settlement_verification;
use celestia_prover::CelestiaProver;
use celestia_types::nmt::Namespace;
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

// Example usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env()
        .add_directive("info".parse().unwrap())
        .add_directive("debug".parse().unwrap())
        .add_directive("error".parse().unwrap()))
    .with_span_events(FmtSpan::FULL)
    .init();

    let node_url = "ws://localhost:26658";
    let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o";
    let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;

    let prover = CelestiaProver::new(node_url, auth_token, namespace).await?;

    // Submit blob and get height
    let height = prover.test_blob_submit().await?;

    // Get verification data
    let verification_data = prover.prepare_verification_data(height).await?;

    println!("Verification data prepared successfully!");
    println!("Start index: {}", verification_data.start_index);
    println!("Data length: {}", verification_data.data_len);
    println!("Data root: {:?}", verification_data.data_root_tuple.data_root);
    println!("Shares proof: {:?}", verification_data.shares_proof);
    println!("Binary proof: {:?}", verification_data.binary_proof);

    // Prepare contract proof data
    let (proof_data, block_number, start_index, data_len) = prover
        .prepare_contract_proof_data(
            verification_data,
            height,
            [0u8; 32], // state_root - replace with actual
            [0u8; 32], // rollup_block_hash - replace with actual
        )
        .await?;

    println!("Contract proof data prepared successfully!");
    println!("Proof data: {:?}", proof_data);
    println!("Block number: {}", block_number);
    println!("Start index: {}", start_index);
    println!("Data length: {}", data_len);

    // Submit to contract
    let contract_address = "0x723464397829ce5ccF1AfAb0b49A59e04f299Fc6";
    let private_key = "0x8167e51f2c57e08b6eabb2ab84a39169527289292d26f62310ee0572d519f97e";

    println!("Submitting proof to contract...");
    let success = prover
        .submit_to_contract(
            contract_address,
            private_key,
            proof_data,
            block_number,
            height,
            start_index,
            data_len,
        )
        .await?;

    if success {
        println!("Proof verification successful!");
    } else {
        println!("Proof verification failed!");
    }

    Ok(())
}
