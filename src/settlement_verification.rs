use crate::celestia_prover::{CelestiaProver, VerificationData};
use alloy::{
    network::EthereumWallet,
    primitives::{Bytes, U256},
    signers::local::PrivateKeySigner,
    sol
};
use alloy_provider::{Provider, ProviderBuilder};
use anyhow::Context;
use celestia_types::nmt::Namespace;
use L1SettlementContract::{BinaryMerkleProof, DataRootTuple, ProofData, SharesProof};

sol! {
    #[sol(rpc)]
    contract L1SettlementContract {

        #[derive(Debug)]
        struct SequenceSpan {
            uint64 height;
            uint64 startIndex;
            uint64 dataLen;
        }

        #[derive(Debug)]
        struct SharesProof {
            bytes[] row_proofs;
        }

        #[derive(Debug)]
        struct DataRootTuple {
            uint256 height;
            bytes32 dataRoot;
        }

        #[derive(Debug)]
        struct BinaryMerkleProof {
            bytes32[] siblings;
            bytes path;
        }

        #[derive(Debug)]
        struct ProofData {
            bytes32 stateRoot;
            bytes32 rollupBlockHash;
            bytes zkProof;
            SharesProof sharesProof;
            uint256 blobstreamNonce;
            DataRootTuple tuple;
            BinaryMerkleProof proof;
        }

        #[derive(Debug)]
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
    pub async fn prepare_contract_proof_data(
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
                .map(|p| Bytes::from(p.into_iter().map(|b| b as u8).collect::<Vec<u8>>()))
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
            // Convert Vec<bool> to bytes by packing 8 bools into each byte
            path: Bytes::from(
                verification_data.binary_proof.path
                    .chunks(8)
                    .map(|chunk| {
                        chunk.iter().enumerate().fold(0u8, |acc, (i, &bit)| {
                            acc | ((bit as u8) << (7 - i))
                        })
                    })
                    .collect::<Vec<u8>>()
            ),
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
        let signer: PrivateKeySigner = private_key
            .trim_start_matches("0x")
            .parse()
            .with_context(|| "Error parsing private key")?;
        let wallet = EthereumWallet::from(signer);
    
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet.clone())
            .on_http(
                "https://ethereum-sepolia-rpc.publicnode.com"
                    .parse()
                    .with_context(|| "Error parsing RPC URL")?,
            );
        
        let contract = L1SettlementContract::new(contract_address.parse().unwrap(), provider.clone());
        
        // Create the transaction data
        let tx_data = contract.submitProof(
            U256::from(block_number),
            celestia_height,
            start_index,
            data_len,
            proof_data.clone()
        );
    
        // Debug print all parameters
        tracing::info!("Transaction parameters:");
        tracing::info!("Block number: {}", block_number);
        tracing::info!("Celestia height: {}", celestia_height);
        tracing::info!("Start index: {}", start_index);
        tracing::info!("Data length: {}", data_len);
        tracing::info!("Proof data state root: {:?}", proof_data.stateRoot);
        tracing::info!("Row proofs length: {}", proof_data.sharesProof.row_proofs.len());
        tracing::info!("Binary proof siblings length: {}", proof_data.proof.siblings.len());
        
        // Try to simulate the transaction first
        let tx_req = tx_data.clone().into_transaction_request();
        match provider.call(&tx_req).await {
            Ok(_) => tracing::info!("Transaction simulation successful"),
            Err(e) => {
                tracing::error!("Transaction simulation failed: {:?}", e);
                return Err(Box::new(e));
            }
        }
    
        // If simulation succeeds, try to estimate gas
        let estimated_gas = match provider.estimate_gas(&tx_req).await {
            Ok(gas) => {
                tracing::info!("Estimated gas: {}", gas);
                gas
            }
            Err(e) => {
                tracing::error!("Gas estimation failed: {:?}", e);
                return Err(Box::new(e));
            }
        };
    
        // Add gas buffer and create final transaction
        let tx_req = tx_data
            .gas(estimated_gas + 50000) // Add buffer to estimated gas
            .max_fee_per_gas(30000000000u128) // 30 gwei
            .max_priority_fee_per_gas(2000000000u128) // 2 gwei
            .into_transaction_request();
    
        let pending_tx = provider
            .send_transaction(tx_req)
            .await
            .with_context(|| "Error sending transaction")?;
    
        tracing::info!("Transaction sent with hash: {}", pending_tx.tx_hash());
    
        // Wait for receipt
        match pending_tx.get_receipt().await {
            Ok(receipt) => {
                let status = receipt.status();
                if !status {
                    tracing::error!("Transaction failed in block {}", receipt.block_number.unwrap_or_default());
                    Ok(false)
                } else {
                    tracing::info!("Transaction succeeded in block {}", receipt.block_number.unwrap_or_default());
                    Ok(true)
                }
            }
            Err(e) => {
                tracing::error!("Failed to get receipt: {:?}", e);
                Err(Box::new(e))
            }
        }
    }
}

// // Example usage
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let node_url = "ws://localhost:26658";
//     let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIiwid3JpdGUiLCJhZG1pbiJdLCJOb25jZSI6IkZaL0hNZFU0S2pTcnFYQTg5THMyaURUdDRkb0xjU1dIcjk5WEV5ajJnalU9IiwiRXhwaXJlc0F0IjoiMDAwMS0wMS0wMVQwMDowMDowMFoifQ.gAZ3VQ7lXL6zsfq0rJtTYJh2yWExI_EYNJ5YnwVRb3o";
//     let namespace = Namespace::new_v0(&[0xDE, 0xAF, 0xBE, 0xEF])?;

//     let prover = CelestiaProver::new(node_url, auth_token, namespace).await?;

//     // Submit blob and get height
//     let height = prover.test_blob_submit().await?;

//     // Get verification data
//     let verification_data = prover.prepare_verification_data(height).await?;

//     // Prepare contract proof data
//     let (proof_data, block_number, start_index, data_len) = prover
//         .prepare_contract_proof_data(
//             verification_data,
//             height,
//             [0u8; 32], // state_root - replace with actual
//             [0u8; 32], // rollup_block_hash - replace with actual
//         )
//         .await?;

//     // Submit to contract
//     let contract_address = "0x723464397829ce5ccF1AfAb0b49A59e04f299Fc6";
//     let private_key = "0x8167e51f2c57e08b6eabb2ab84a39169527289292d26f62310ee0572d519f97e";

//     let success = prover
//         .submit_to_contract(
//             contract_address,
//             private_key,
//             proof_data,
//             block_number,
//             height,
//             start_index,
//             data_len,
//         )
//         .await?;

//     if success {
//         println!("Proof verification successful!");
//     } else {
//         println!("Proof verification failed!");
//     }

//     Ok(())
// }
