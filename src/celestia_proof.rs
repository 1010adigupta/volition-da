use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::{
    blob::Blob, header::ExtendedHeader, nmt::Namespace, row_namespace_data::NamespaceData,
    share::Share,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use celestia_rpc::CelestiaTest;
// Structures to match the contract's requirements
#[derive(Debug, Serialize, Deserialize)]
struct SharesProof {
    row_proofs: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DataRootTuple {
    height: u64,
    data_root: [u8; 32],
}

#[derive(Debug, Serialize, Deserialize)]
struct BinaryMerkleProof {
    siblings: Vec<[u8; 32]>,
    path: Vec<bool>,
}

#[derive(Debug)]
struct VerificationData {
    shares_proof: SharesProof,
    data_root_tuple: DataRootTuple,
    binary_proof: BinaryMerkleProof,
    start_index: u64,
    data_len: u64,
}

struct CelestiaProver {
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

    // Get shares proof from namespace data
    async fn get_shares_proof(
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
                // Convert the proof to bytes
                row_proofs.push(row.proof.to_bytes()?);
            }
        }

        // Calculate start_index and data_len
        let (start_index, data_len) = calculate_share_range(&namespace_data);

        Ok((SharesProof { row_proofs }, start_index, data_len))
    }

    // Get data root tuple
    async fn get_data_root_tuple(&self, height: u64) -> Result<DataRootTuple, Box<dyn Error>> {
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
    async fn get_merkle_proof(&self, height: u64) -> Result<BinaryMerkleProof, Box<dyn Error>> {
        let header = self.client.header_get_by_height(height).await?;

        // Get commitment for the namespace
        let blobs = self.client.blob_get_all(height, &[self.namespace]).await?;
        let commitment = blobs
            .unwrap_or_default()
            .first()
            .ok_or("No blob found")?
            .commitment
            .clone();

        // Get proof for the commitment
        let proof = self
            .client
            .blob_get_proof(height, self.namespace, commitment)
            .await?;

        Ok(BinaryMerkleProof {
            siblings: proof.proof.siblings.into_iter().map(|s| s.into()).collect(),
            path: proof.proof.path,
        })
    }

    // Main function to prepare all verification data
    pub async fn prepare_verification_data(
        &self,
        height: u64,
    ) -> Result<VerificationData, Box<dyn Error>> {
        // Get all proofs in parallel using tokio::join!
        let (shares_result, root_result, merkle_result) = tokio::join!(
            self.get_shares_proof(height),
            self.get_data_root_tuple(height),
            self.get_merkle_proof(height)
        );

        let (shares_proof, start_index, data_len) = shares_result?;
        let data_root_tuple = root_result?;
        let binary_proof = merkle_result?;

        Ok(VerificationData {
            shares_proof,
            data_root_tuple,
            binary_proof,
            start_index,
            data_len,
        })
    }
}

// Helper function from your existing code
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

// Usage example
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let node_url = "ws://localhost:26658";
    let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o";
    let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;
    let test = CelestiaTest::new().await?;
    
    // Submit a blob and get its height
    let height = test.test_blob_submit().await?;

    let prover = CelestiaProver::new(node_url, auth_token, namespace).await?;

    // Get the block height where your data was submitted
    let height = 1000; // Example height

    // Get all verification data
    let verification_data = prover.prepare_verification_data(height).await?;

    println!("Verification data prepared successfully!");
    println!("Start index: {}", verification_data.start_index);
    println!("Data length: {}", verification_data.data_len);

    Ok(())
}
