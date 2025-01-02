use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::{
    blob::Blob,
    nmt::{Namespace, NamespaceProof, NamespacedSha2Hasher, NS_SIZE},
    row_namespace_data::NamespaceData,
    ExtendedHeader,
    hash::Hash,
    TxConfig,
    consts::appconsts
};
use nmt_rs::nmt_proof::NamespaceProof as NmtNamespaceProof;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
// Structures to match the contract's requirements
#[derive(Debug, Serialize, Deserialize)]
pub struct SharesProof {
    pub row_proofs: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataRootTuple {
    pub height: u64,
    pub data_root: [u8; 32],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BinaryMerkleProof {
    pub siblings: Vec<[u8; 32]>,
    pub path: Vec<bool>,
}

#[derive(Debug)]
pub struct VerificationData {
    pub shares_proof: SharesProof,
    pub data_root_tuple: DataRootTuple,
    pub binary_proof: BinaryMerkleProof,
    pub start_index: u64,
    pub data_len: u64,
}

pub struct CelestiaProver {
    client: Client,
    namespace: Namespace,
}

impl CelestiaProver {
    pub async fn new(
        node_url: &str,
        auth_token: &str,
        namespace: Namespace,
    ) -> Result<Self, Box<dyn Error>> {
        let client = Client::new(node_url, Some(auth_token)).await?;
        Ok(Self { client, namespace })
    }

    pub async fn get_shares_proof(
        &self,
        height: u64,
    ) -> Result<(SharesProof, u64, u64), Box<dyn Error>> {
        // Get the header for this height
        let header = self.client.header_get_by_height(height).await?;
    
        // Get namespace data
        let namespace_data = self
            .client
            .share_get_namespace_data(&header, self.namespace)
            .await?;
    
        // Extract row proofs and calculate indices
        let mut row_proofs = Vec::new();
        for row in &namespace_data.rows {
            if !row.shares.is_empty() {
                // Serialize the proof directly to bytes
                let proof_bytes = serde_json::to_vec(&row.proof)?;
                row_proofs.push(proof_bytes);
            }
        }
    
        // Calculate start_index and data_len
        let (start_index, data_len) = calculate_share_range(&namespace_data);
    
        Ok((SharesProof { row_proofs }, start_index, data_len))
    }

    // Get data root tuple
    pub async fn get_data_root_tuple(&self, height: u64) -> Result<DataRootTuple, Box<dyn Error>> {
        let header = self.client.header_get_by_height(height).await?;
        // Get the complete data root hash from DAH by hashing all row and column roots
        let Hash::Sha256(root) = header.dah.hash() else {
            return Err("Failed to get hash".into());
        };
        Ok(DataRootTuple {
            height,
            data_root: root, // Convert directly to bytes
        })
    }

    // Get binary Merkle proof
    pub async fn get_merkle_proof(&self, height: u64) -> Result<BinaryMerkleProof, Box<dyn Error>> {
        let header = self.client.header_get_by_height(height).await?;
        
        let namespace_data = self.client
            .share_get_namespace_data(&header, self.namespace)
            .await?;
    
        if namespace_data.rows.is_empty() {
            return Err("No data found for namespace".into());
        }
    
        // Get the first valid row's proof
        let first_proof = namespace_data.rows.iter()
            .find(|row| !row.shares.is_empty())
            .ok_or("No valid proofs found")?
            .proof
            .clone();
    
        // Get the inner proof
        let nmt_proof = first_proof.into_inner();
    
        // Extract siblings and path based on the type of proof
        match nmt_proof {
            NmtNamespaceProof::PresenceProof { proof, .. } => {
                Ok(BinaryMerkleProof {
                    siblings: proof.siblings.into_iter().map(|s| s.hash()).collect(),
                    path: proof.range.into_iter().map(|idx| idx % 2 == 1).collect(),
                })
            },
            NmtNamespaceProof::AbsenceProof { proof, .. } => {
                Ok(BinaryMerkleProof {
                    siblings: proof.siblings.into_iter().map(|s| s.hash()).collect(),
                    path: proof.range.into_iter().map(|idx| idx % 2 == 1).collect(),
                })
            }
        }
    }

    // Main function to prepare all verification data
    pub async fn prepare_verification_data(
        &self,
        height: u64,
    ) -> Result<VerificationData, Box<dyn Error>> {
        let shares_proof = self.get_shares_proof(height).await?;
        let data_root_tuple = self.get_data_root_tuple(height).await?;
        let binary_proof = self.get_merkle_proof(height).await?;

        Ok(VerificationData {
            shares_proof: SharesProof { row_proofs: shares_proof.0.row_proofs },
            data_root_tuple,
            binary_proof,
            start_index: shares_proof.1,
            data_len: shares_proof.2,
        })
    }

    pub async fn test_blob_submit(&self) -> Result<u64, Box<dyn std::error::Error>> {
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
}

// Helper function from your existing code
pub fn calculate_share_range(namespace_data: &NamespaceData) -> (u64, u64) {
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

// Usage example
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let node_url = "ws://localhost:26658";
    let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o";
    let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;
    println!("Submitting blob and getting height.....");

    let prover = CelestiaProver::new(node_url, auth_token, namespace).await?;

    // Submit a blob and get its height
    let height = prover.test_blob_submit().await?;

    // Get all verification data
    let verification_data = prover.prepare_verification_data(height).await?;

    println!("Verification data prepared successfully!");
    println!("Start index: {}", verification_data.start_index);
    println!("Data length: {}", verification_data.data_len);
    println!("Data root: {:?}", verification_data.data_root_tuple.data_root);
    println!("Shares proof: {:?}", verification_data.shares_proof);
    println!("Binary proof: {:?}", verification_data.binary_proof);

    Ok(())
}
