use alloy::primitives::{Address, B256};
use alloy::signers::Signature;
use anyhow::{Context, Result};

/// Verify a wallet signature
/// Returns Ok(()) if signature is valid for the given message_hash
pub fn verify_signature(
    address: &str,
    signature_hex: &str,
    message_hash: &str,
) -> Result<bool> {
    // Parse address
    let expected_addr = Address::from_str(address)
        .context("Invalid wallet address")?;

    // Parse signature
    let sig_bytes =
        hex::decode(signature_hex.trim_start_matches("0x"))
            .context("Invalid signature hex")?;

    if sig_bytes.len() != 65 {
        return Ok(false);
    }

    let sig = Signature::from_bytes_and_parity(
        &sig_bytes[..64],
        sig_bytes[64] != 0,
    )
    .context("Invalid signature")?;

    // Verify: signers address from signature should match
    let msg_b256 = B256::from_str(message_hash)
        .context("Invalid message hash")?;

    // In alloy 2.1.x, recover_address_from_msg takes &[u8]
    // We need to hash the message first with keccak256
    let msg_hash_bytes = alloy::primitives::keccak256(&msg_b256.0);
    let recovered = sig
        .recover_address_from_msg(&msg_hash_bytes)
        .context("Failed to recover address")?;

    Ok(recovered == expected_addr)
}
