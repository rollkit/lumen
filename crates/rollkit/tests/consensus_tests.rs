//! Tests for Rollkit consensus implementation

use lumen_rollkit::consensus::RollkitConsensus;
use reth_chainspec::MAINNET;
use reth_consensus::{ConsensusError, HeaderValidator};
use reth_primitives::{Header, SealedHeader};

fn create_test_header(number: u64, parent_hash: [u8; 32], timestamp: u64) -> SealedHeader {
    let mut header = Header::default();
    header.number = number;
    header.parent_hash = parent_hash.into();
    header.timestamp = timestamp;
    header.gas_limit = 30_000_000; // Set a reasonable gas limit
    header.gas_used = 0;

    SealedHeader::new(header, [0u8; 32].into())
}

#[test]
fn test_rollkit_consensus_allows_same_timestamp() {
    let chain_spec = MAINNET.clone();
    let consensus = RollkitConsensus::new(chain_spec);

    // Create parent block
    let parent = create_test_header(1, [0u8; 32], 1000);

    // Create child block with SAME timestamp (this should be allowed)
    let mut child_header = Header::default();
    child_header.number = 2;
    child_header.parent_hash = parent.hash();
    child_header.timestamp = 1000; // Same as parent
    child_header.gas_limit = 30_000_000;
    child_header.gas_used = 0;
    let child = SealedHeader::new(child_header, [1u8; 32].into());

    // This should NOT return an error for Rollkit consensus
    let result = consensus.validate_header_against_parent(&child, &parent);
    if let Err(e) = &result {
        eprintln!("Error validating same timestamp: {:?}", e);
    }
    assert!(
        result.is_ok(),
        "Rollkit consensus should allow same timestamp"
    );
}

#[test]
fn test_rollkit_consensus_rejects_past_timestamp() {
    let chain_spec = MAINNET.clone();
    let consensus = RollkitConsensus::new(chain_spec);

    // Create parent block
    let parent = create_test_header(1, [0u8; 32], 1000);

    // Create child block with timestamp in the past
    let mut child_header = Header::default();
    child_header.number = 2;
    child_header.parent_hash = parent.hash();
    child_header.timestamp = 999; // Less than parent
    child_header.gas_limit = 30_000_000;
    child_header.gas_used = 0;
    let child = SealedHeader::new(child_header, [1u8; 32].into());

    // This should return an error
    let result = consensus.validate_header_against_parent(&child, &parent);
    assert!(
        result.is_err(),
        "Rollkit consensus should reject past timestamp"
    );

    match result {
        Err(ConsensusError::TimestampIsInPast {
            parent_timestamp,
            timestamp,
        }) => {
            assert_eq!(parent_timestamp, 1000);
            assert_eq!(timestamp, 999);
        }
        _ => panic!("Expected TimestampIsInPast error"),
    }
}

#[test]
fn test_rollkit_consensus_allows_future_timestamp() {
    let chain_spec = MAINNET.clone();
    let consensus = RollkitConsensus::new(chain_spec);

    // Create parent block
    let parent = create_test_header(1, [0u8; 32], 1000);

    // Create child block with future timestamp
    let mut child_header = Header::default();
    child_header.number = 2;
    child_header.parent_hash = parent.hash();
    child_header.timestamp = 1001; // Greater than parent
    child_header.gas_limit = 30_000_000; // Same gas limit as parent
    child_header.gas_used = 0;
    let child = SealedHeader::new(child_header, [1u8; 32].into());

    // This should be allowed
    let result = consensus.validate_header_against_parent(&child, &parent);
    if let Err(e) = &result {
        eprintln!("Error validating future timestamp: {e:?}");
    }
    assert!(
        result.is_ok(),
        "Rollkit consensus should allow future timestamp"
    );
}

#[test]
fn test_rollkit_consensus_validates_parent_hash() {
    let chain_spec = MAINNET.clone();
    let consensus = RollkitConsensus::new(chain_spec);

    // Create parent block
    let parent = create_test_header(1, [0u8; 32], 1000);

    // Create child block with wrong parent hash
    let mut child_header = Header::default();
    child_header.number = 2;
    child_header.parent_hash = [99u8; 32].into(); // Wrong parent hash
    child_header.timestamp = 1000;
    child_header.gas_limit = 30_000_000;
    child_header.gas_used = 0;
    let child = SealedHeader::new(child_header, [1u8; 32].into());

    // This should return an error
    let result = consensus.validate_header_against_parent(&child, &parent);
    assert!(
        result.is_err(),
        "Rollkit consensus should validate parent hash"
    );
}

#[test]
fn test_rollkit_consensus_validates_block_number() {
    let chain_spec = MAINNET.clone();
    let consensus = RollkitConsensus::new(chain_spec);

    // Create parent block
    let parent = create_test_header(1, [0u8; 32], 1000);

    // Create child block with wrong block number
    let mut child_header = Header::default();
    child_header.number = 3; // Should be 2
    child_header.parent_hash = parent.hash();
    child_header.timestamp = 1000;
    child_header.gas_limit = 30_000_000;
    child_header.gas_used = 0;
    let child = SealedHeader::new(child_header, [1u8; 32].into());

    // This should return an error
    let result = consensus.validate_header_against_parent(&child, &parent);
    assert!(
        result.is_err(),
        "Rollkit consensus should validate block number"
    );
}
