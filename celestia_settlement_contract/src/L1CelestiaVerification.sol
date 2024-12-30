// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;  // Changed to a stable version

import "blobstream-contracts/IDAOracle.sol";
import "blobstream-contracts/DataRootTuple.sol";
import "blobstream-contracts/lib/verifier/DAVerifier.sol";
import "blobstream-contracts/lib/tree/binary/BinaryMerkleProof.sol";

contract ZKRollupSettlement {
    IDAOracle public immutable blobstream;
    bytes32[] public rollupBlockHashes;
    
    struct Block {
        uint256 blockNumber;
        bytes32 stateRoot;
        bytes32 previousBlockHash;
        SequenceSpan span;
    }
    
    struct SequenceSpan {
        uint64 height;
        uint64 startIndex;
        uint64 dataLen;
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
    
    mapping(uint256 => Block) public blocks;
    uint256 public latestBlock;
    
    event BlockVerified(uint256 blockNumber, bytes32 stateRoot, bytes32 rollupBlockHash);
    
    constructor(address _blobstream) {
        blobstream = IDAOracle(_blobstream);
    }

    function _verifyAttestationAndData(
        ProofData memory data,
        uint256 blobstreamNonce
    ) internal view returns (bool) {
        // Verify the attestation in Blobstream contract
        if (!blobstream.verifyAttestation(blobstreamNonce, data.tuple, data.proof)) {
            return false;
        }
        
        // Verify data inclusion using DAVerifier
        (bool valid, DAVerifier.ErrorCodes err) = DAVerifier.verifySharesToDataRootTupleRoot(
            blobstream,
            data.sharesProof
        );
        return valid;
    }

    function _verifySpanAndHeight(
        SequenceSpan memory span,
        ProofData memory data
    ) internal pure returns (bool) {
        // Verify sequence span matches tuple
        if (span.height != data.tuple.height) {
            return false;
        }

        // Verify span bounds
        (uint256 squareSize, DAVerifier.ErrorCodes err) = DAVerifier.computeSquareSizeFromRowProof(
            data.sharesProof.rowProofs[0]
        );
        if (err != DAVerifier.ErrorCodes.NoError) {
            return false;
        }
        
        uint256 maxIndex = 4 * squareSize;
        return span.startIndex + span.dataLen <= maxIndex;
    }
    
    function submitProof(
        uint256 blockNumber,
        uint64 celestiaHeight,
        uint64 startIndex,
        uint64 dataLen,
        ProofData calldata proofData
    ) external {
        // Create span structure
        SequenceSpan memory span = SequenceSpan({
            height: celestiaHeight,
            startIndex: startIndex,
            dataLen: dataLen
        });

        // Verify attestation and data
        require(
            _verifyAttestationAndData(proofData, proofData.blobstreamNonce),
            "Invalid attestation or data"
        );
        
        // Verify span and height
        require(
            _verifySpanAndHeight(span, proofData),
            "Invalid span or height"
        );

        // Verify ZKP
        require(
            verifyZKP(proofData.rollupBlockHash, proofData.zkProof, proofData.tuple.dataRoot),
            "Invalid ZKP"
        );
        
        // Store block data
        blocks[blockNumber] = Block({
            blockNumber: blockNumber,
            stateRoot: proofData.stateRoot,
            previousBlockHash: blocks[blockNumber - 1].stateRoot,
            span: span
        });
        
        rollupBlockHashes.push(proofData.rollupBlockHash);
        latestBlock = blockNumber;
        
        emit BlockVerified(blockNumber, proofData.stateRoot, proofData.rollupBlockHash);
    }

    function verifyZKP(
        bytes32 rollupBlockHash,
        bytes calldata zkProof,
        bytes32 dataRoot
    ) private pure returns (bool) {
        // Dummy implementation for testing
        // In production, this would verify that the rollup block hash
        // correctly commits to the state transition and data root
        return true;
    }
}