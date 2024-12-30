use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::{nmt::Namespace, Blob, TxConfig, consts::appconsts, row_namespace_data::NamespaceData};
use tokio;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CelestiaTest {
    client: Client,
    namespace: Namespace,
}

impl CelestiaTest {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let node_url = "ws://localhost:26658";
        let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o";

        let client = Client::new(node_url, Some(auth_token)).await?;
        let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;

        Ok(Self { client, namespace })
    }

    async fn test_blob_submit(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let message = format!("Hello Celestians! Timestamp: {}", timestamp);
        println!("Preparing to submit message: {}", message);
        
        let blob = Blob::new(
            self.namespace,
            message.as_bytes().to_vec(),
            appconsts::AppVersion::V2,
        )?;
    
        let tx_config = TxConfig::default();
        let height = self.client.blob_submit(&[blob.clone()], tx_config).await?;
        
        println!("Successfully submitted blob at height: {}", height);
        Ok(height)
    }

    async fn test_blob_get_all(&self, height: u64) -> Result<(), Box<dyn std::error::Error>> {
        println!("Waiting for blob to be processed...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
        match self.client.blob_get_all(height, &[self.namespace]).await {
            Ok(Some(blobs)) if !blobs.is_empty() => {
                println!("\nBlob verification successful:");
                println!("Retrieved blob data: {}", String::from_utf8_lossy(&blobs[0].data));
                println!("Namespace: {:?}", blobs[0].namespace);
                println!("Share version: {:?}", blobs[0].share_version);
                println!("Commitment: {:?}", blobs[0].commitment);
            }
            Ok(Some(_)) => println!("No blobs found at height {} for the specified namespace", height),
            Ok(None) => println!("No blobs found at height {}", height),
            Err(e) => eprintln!("Failed to retrieve blobs: {}", e),
        }
    
        Ok(())
    }

    async fn test_share_get_namespace_data(&self, height: u64) -> Result<(), Box<dyn std::error::Error>> {
        println!("Testing share_get_namespace_data...");
        
        // First get the header for this height
        let header = self.client.header_get_by_height(height).await?;
        
        match self.client.share_get_namespace_data(&header, self.namespace).await {
            Ok(shares) => {
                println!("\nNamespace shares retrieved successfully:");
                let (start_index, total_shares) = calculate_share_range(&shares);
                println!("Start index: {}", start_index);
                println!("Total shares: {}", total_shares);
            }
            Err(e) => eprintln!("Failed to retrieve namespace shares: {}", e),
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let test = CelestiaTest::new().await?;
    
    // Submit a blob and get its height
    let height = test.test_blob_submit().await?;
    
    // Test blob retrieval
    test.test_blob_get_all(height).await?;
    
    // Test namespace shares retrieval
    test.test_share_get_namespace_data(height).await?;

    Ok(())
}

fn calculate_share_range(namespace_data: &NamespaceData) -> (u64, u64) {
    if namespace_data.rows.is_empty() {
        return (0, 0);
    }

    let mut start_index = u64::MAX;
    let mut total_shares = 0u64;

    // Iterate through each row to find the start index and count total shares
    for row in &namespace_data.rows {
        if !row.shares.is_empty() {
            // Get start index from the namespace proof's start index
            let absolute_index = row.proof.start_idx() as u64;
            
            // Update the global start index if this is the earliest we've seen
            start_index = start_index.min(absolute_index);
            
            // Add the number of shares in this row to the total
            total_shares += row.shares.len() as u64;
        }
    }

    // If we never found a valid start index (no shares found), reset to 0
    if start_index == u64::MAX {
        start_index = 0;
        total_shares = 0;
    }

    (start_index, total_shares)
}
