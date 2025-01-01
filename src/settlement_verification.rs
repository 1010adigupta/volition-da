use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_sol_types::{sol, SolCall};
use ethers_core::types::TransactionRequest;
use std::str::FromStr;
use celestia_types::nmt::Namespace;
use crate::celestia_prover::{CelestiaProver, VerificationData};
// Define the Solidity contract structures
sol! {
    struct SequenceSpan {
        uint64 height;
        uint64 startIndex;
        uint64 dataLen;
    }

    struct SharesProof {
        bytes[] row_proofs;
    }

    struct DataRootTuple {
        uint256 height;
        bytes32 dataRoot;
    }

    struct BinaryMerkleProof {
        bytes32[] siblings;
        bytes path;
    }

    struct ProofData {
        bytes32 stateRoot;
        bytes32 rollupBlockHash;
        bytes zkProof;
        SharesProof sharesProof;
        uint256 blobstreamNonce;
        DataRootTuple tuple;
        BinaryMerkleProof proof;
    }

    interface IZKRollupSettlement {
        function submitProof(
            uint256 blockNumber,
            uint64 celestiaHeight,
            uint64 startIndex,
            uint64 dataLen,
            ProofData calldata proofData
        ) external;
    }
}

impl CelestiaProver {
    // Function to convert our proof data to contract format
    async fn prepare_contract_proof_data(
        &self,
        verification_data: VerificationData,
        block_number: u64,
        state_root: [u8; 32],
        rollup_block_hash: [u8; 32],
    ) -> Result<(ProofData, u64, u64, u64), Box<dyn std::error::Error>> {
        // Convert SharesProof
        let shares_proof = SharesProof {
            row_proofs: verification_data
                .shares_proof
                .row_proofs
                .into_iter()
                .map(|p| Bytes::from(p))
                .collect(),
        };

        // Convert BinaryMerkleProof
        let binary_proof = BinaryMerkleProof {
            siblings: verification_data
                .binary_proof
                .siblings
                .into_iter()
                .map(|s| s.into())
                .collect(),
            path: Bytes::from(verification_data.binary_proof.path),
        };

        // Convert DataRootTuple
        let tuple = DataRootTuple {
            height: U256::from(verification_data.data_root_tuple.height),
            dataRoot: verification_data.data_root_tuple.data_root.into(),
        };

        // Create ProofData struct
        let proof_data = ProofData {
            stateRoot: state_root.into(),
            rollupBlockHash: rollup_block_hash.into(),
            zkProof: Bytes::default(), // Empty for testing
            sharesProof: shares_proof,
            blobstreamNonce: U256::from(block_number),
            tuple,
            proof: binary_proof,
        };

        Ok((
            proof_data,
            block_number,
            verification_data.start_index,
            verification_data.data_len,
        ))
    }

    // Function to submit proof to the contract
    pub async fn submit_to_contract(
        &self,
        contract_address: &str,
        private_key: &str,
        proof_data: ProofData,
        block_number: u64,
        celestia_height: u64,
        start_index: u64,
        data_len: u64,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Setup provider
        let provider = ProviderBuilder::new()
            .rpc_url("https://eth-sepolia.g.alchemy.com/v2/YOUR_API_KEY")
            .build()?;

        // Setup wallet
        let wallet = LocalWallet::from_str(private_key)?.with_chain_id(Chain::Sepolia as u64);

        // Create contract instance
        let contract_addr = Address::from_str(contract_address)?;

        // Create call data
        let call = IZKRollupSettlement::submitProofCall {
            blockNumber: U256::from(block_number),
            celestiaHeight: U256::from(celestia_height),
            startIndex: U256::from(start_index),
            dataLen: U256::from(data_len),
            proofData: proof_data,
        };

        // Encode call data
        let calldata = call.encode();

        // Create transaction
        let tx = TransactionRequest::new()
            .to(contract_addr)
            .data(calldata)
            .from(wallet.address());

        // Send transaction
        let pending_tx = provider.send_transaction(tx, None).await?;

        // Wait for confirmation
        let receipt = pending_tx.await?;

        // Check if transaction was successful
        Ok(receipt.status.unwrap_or_default().as_u64() == 1)
    }
}

// Example usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let node_url = "ws://localhost:26658";
    let auth_token = "your_auth_token";
    let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;

    let prover = CelestiaProver::new(node_url, auth_token, namespace).await?;

    // Submit blob and get height
    let height = prover.test_blob_submit().await?;

    // Get verification data
    let verification_data = prover.prepare_verification_data(height).await?;

    // Prepare contract proof data
    let (proof_data, block_number, start_index, data_len) = prover
        .prepare_contract_proof_data(
            verification_data,
            height,
            [0u8; 32], // state_root - replace with actual
            [0u8; 32], // rollup_block_hash - replace with actual
        )
        .await?;

    // Submit to contract
    let contract_address = "YOUR_CONTRACT_ADDRESS";
    let private_key = "YOUR_PRIVATE_KEY";

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
