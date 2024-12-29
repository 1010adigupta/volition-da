// src/poster.rs
use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::{nmt::Namespace, Blob, TxConfig, consts::appconsts, row_namespace_data::NamespaceData};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SequenceSpan {
    pub height: u64,
    pub start_index: u64,
    pub data_len: u64,
}

pub struct CelestiaClient {
    client: Client,
    namespace: Namespace,
}

impl CelestiaClient {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let node_url = "ws://localhost:26658";
        let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o";

        let client = Client::new(node_url, Some(auth_token)).await?;
        let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;

        Ok(Self { client, namespace })
    }

    pub async fn submit_pfb(&self, data: Vec<u8>) -> Result<SequenceSpan, Box<dyn std::error::Error>> {
        let blob = Blob::new(
            self.namespace,
            data,
            appconsts::AppVersion::V2,
        )?;

        let tx_config = TxConfig::default();
        let height = self.client.blob_submit(&[blob.clone()], tx_config).await?;
        
        // Get namespace data to find start index
        let header = self.client.header_get_by_height(height).await?;
        let namespace_data = self.client.share_get_namespace_data(&header, self.namespace).await?;
        
        let (start_index, total_shares) = calculate_share_range(&namespace_data);
        
        Ok(SequenceSpan {
            height,
            start_index,
            data_len: total_shares,
        })
    }

    pub async fn get_shares_by_height(&self, height: u64) -> Result<NamespaceData, Box<dyn std::error::Error>> {
        let header = self.client.header_get_by_height(height).await?;
        let namespace_data = self.client.share_get_namespace_data(&header, self.namespace).await?;
        Ok(namespace_data)
    }
}

fn calculate_share_range(namespace_data: &NamespaceData) -> (u64, u64) {
    if namespace_data.rows.is_empty() {
        return (0, 0);
    }

    let mut start_index = u64::MAX;
    let mut total_shares = 0u64;

    for row in &namespace_data.rows {
        if !row.shares.is_empty() {
            let absolute_index = row.proof.start_idx() as u64;
            start_index = start_index.min(absolute_index);
            total_shares += row.shares.len() as u64;
        }
    }

    if start_index == u64::MAX {
        start_index = 0;
        total_shares = 0;
    }

    (start_index, total_shares)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_pfb() -> Result<(), Box<dyn std::error::Error>> {
        let client = CelestiaClient::new().await?;

        let test_data = b"Hello Celestia!".to_vec();
        let span = client.submit_pfb(test_data).await?;
        
        println!("Block submitted at height: {}", span.height);
        println!("Start index: {}", span.start_index);
        println!("Total shares: {}", span.data_len);
        
        // Verify we can retrieve the data
        let namespace_data = client.get_shares_by_height(span.height).await?;
        assert!(!namespace_data.rows.is_empty(), "Should have retrieved shares");
        
        Ok(())
    }
}