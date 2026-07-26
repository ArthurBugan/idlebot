use alloy::primitives::{Address, B256};
use alloy::signers::Signature;
use anyhow::{Context, Result};
use std::str::FromStr;

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

    // In alloy 2.1.x+, use Signature::try_from for fixed-size bytes
    let sig = Signature::try_from(&sig_bytes[..65])
        .context("Invalid signature")?;

    // Verify: signers address from signature should match
    let msg_b256 = B256::from_str(message_hash)
        .context("Invalid message hash")?;

    let recovered = sig
        .recover_address_from_msg(&msg_b256)
        .context("Failed to recover address")?;

    Ok(recovered == expected_addr)
}
