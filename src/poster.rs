// src/celestia_client.rs
use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[derive(Debug, Serialize, Deserialize)]
pub struct Namespace {
    pub version: u8,
    pub id: [u8; 28],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SequenceSpan {
    pub height: u64,
    pub start_index: u64,
    pub data_len: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PfbResponse {
    height: u64,
    txhash: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SharesResponse {
    shares: Vec<String>,
    height: u64,
    start_share: u64,
    end_share: u64,
}

pub struct CelestiaClient {
    client: Client,
    endpoint: String,
    namespace_id: Namespace,
    auth_token: String,
}

impl CelestiaClient {
    pub fn new(endpoint: String, namespace_id: Namespace, auth_token: String) -> Self {
        Self {
            client: Client::new(),
            endpoint,
            namespace_id,
            auth_token,
        }
    }

    pub async fn submit_pfb(&self, data: Vec<u8>) -> Result<SequenceSpan, Box<dyn std::error::Error>> {
        // Convert data to base64
        let data_base64 = BASE64.encode(&data);
        
        // Convert namespace ID to base64
        let namespace_base64 = BASE64.encode(&self.namespace_id.id);

        // Prepare request body
        let body = json!({
            "namespace_id": namespace_base64,
            "data": data_base64,
            "gas_limit": 180000,
            "fee": 2000, // Adjust fee as needed
        });

        // Submit PFB transaction
        let response = self.client
            .post(format!("{}/submit_pfb", self.endpoint))
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&body)
            .send()
            .await?;

        println!("Response status: {}", response.status());
        let pfb_result: PfbResponse = response.json().await?;
        println!("Response body: {:?}", pfb_result);

        // Get share range for the transaction
        let shares = self.get_shares_by_tx(pfb_result.txhash).await?;

        Ok(SequenceSpan {
            height: shares.height,
            start_index: shares.start_share,
            data_len: shares.end_share - shares.start_share,
        })
    }

    async fn get_shares_by_tx(&self, txhash: String) -> Result<SharesResponse, Box<dyn std::error::Error>> {
        // Wait for transaction to be included in a block
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let response = self.client
            .get(format!("{}/namespaced_shares/{}/{}", self.endpoint, BASE64.encode(&self.namespace_id.id), txhash))
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await?;

        let shares: SharesResponse = response.json().await?;
        Ok(shares)
    }
}

// Helper function to create a namespace ID from a string
pub fn create_namespace(input: &str) -> Namespace {
    let mut id = [0u8; 28];
    let bytes = input.as_bytes();
    let len = std::cmp::min(bytes.len(), 28);
    id[..len].copy_from_slice(&bytes[..len]);
    
    Namespace {
        version: 0,
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_pfb() {
        let namespace = create_namespace("test-namespace");
        let client = CelestiaClient::new(
            "http://localhost:26659".to_string(),
            namespace,
            "test-token".to_string(),
        );

        let test_data = b"Test data".to_vec();
        let result = client.submit_pfb(test_data).await;
        assert!(result.is_ok());
    }
}