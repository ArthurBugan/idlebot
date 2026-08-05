//! Wallet Authentication — Nonce generation, signature verification, and session management.

use rand::Rng;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Nonce TTL in seconds (5 minutes)
const NONCE_TTL: u64 = 300;

/// Session TTL in seconds (24 hours)
const SESSION_TTL: u64 = 86400;

/// Stored nonce with expiration
#[derive(Debug, Clone)]
pub struct StoredNonce {
    pub nonce: String,
    pub expires_at: u64,
}

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    pub address: String,
    pub player_id: u64,
    pub created_at: u64,
    pub expires_at: u64,
    pub jwt: String,
}

/// Wallet authentication manager
pub struct WalletAuth {
    /// Nonce storage: address -> StoredNonce
    nonces: HashMap<String, StoredNonce>,
    /// Session storage: address -> Session
    sessions: HashMap<String, Session>,
    /// Rate limiting: address -> count
    rate_limits: HashMap<String, u64>,
    /// Next player ID
    next_player_id: u64,
}

impl WalletAuth {
    /// Create a new WalletAuth manager
    pub fn new() -> Self {
        Self {
            nonces: HashMap::new(),
            sessions: HashMap::new(),
            rate_limits: HashMap::new(),
            next_player_id: 1,
        }
    }

    /// Generate a random nonce (32 bytes, hex-encoded)
    pub fn generate_nonce(&mut self, address: &str) -> String {
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let nonce = hex::encode(&bytes);
        
        let now = Self::now_secs();
        self.nonces.insert(
            address.to_lowercase(),
            StoredNonce {
                nonce: nonce.clone(),
                expires_at: now + NONCE_TTL,
            },
        );
        
        nonce
    }

    /// Verify that a nonce is valid and not expired
    pub fn verify_nonce(&self, address: &str, nonce: &str) -> bool {
        let addr_lower = address.to_lowercase();
        if let Some(stored) = self.nonces.get(&addr_lower) {
            if stored.nonce == nonce && stored.expires_at > Self::now_secs() {
                return true;
            }
        }
        false
    }

    /// Consume a nonce (mark as used to prevent replay)
    pub fn consume_nonce(&mut self, address: &str) {
        let addr_lower = address.to_lowercase();
        self.nonces.remove(&addr_lower);
    }

    /// Verify signature (mock implementation — real impl would use secp256k1)
    /// Returns the recovered address if valid
    pub fn verify_signature(&self, nonce: &str, _signature: &str) -> Option<String> {
        // In production, this would:
        // 1. Prepend EIP-191 prefix "\x19Ethereum Signed Message:\n32"
        // 2. Hash the message with keccak256
        // 3. Recover the signer address from the signature
        // For now, derive address from nonce (deterministic mock)
        let addr = format!("0x{}", &nonce[..40]);
        Some(addr)
    }

    /// Create a session for a player
    pub fn create_session(&mut self, address: &str, player_id: u64) -> Session {
        let now = Self::now_secs();
        let jwt = format!("session_{}_{}", player_id, now); // Mock JWT
        
        let session = Session {
            address: address.to_lowercase(),
            player_id,
            created_at: now,
            expires_at: now + SESSION_TTL,
            jwt,
        };
        
        self.sessions.insert(address.to_lowercase(), session.clone());
        session
    }

    /// Verify a session token is valid
    pub fn verify_session(&self, address: &str, jwt: &str) -> Option<&Session> {
        let addr_lower = address.to_lowercase();
        if let Some(session) = self.sessions.get(&addr_lower) {
            if session.jwt == jwt && session.expires_at > Self::now_secs() {
                return Some(session);
            }
        }
        None
    }

    /// Handle login: verify signature and create/retrieve session
    pub fn handle_login(&mut self, address: &str, nonce: &str, _signature: &str) -> Result<Session, String> {
        // Rate limiting
        let addr_lower = address.to_lowercase();
        let count = self.rate_limits.entry(addr_lower.clone()).or_insert(0);
        if *count >= 5 {
            return Err("Rate limit exceeded".to_string());
        }
        *count += 1;
        
        // Verify nonce
        if !self.verify_nonce(&addr_lower, nonce) {
            return Err("Invalid or expired nonce".to_string());
        }
        
        // Verify signature (mock — always succeeds for valid nonce)
        // In production, this would recover the actual signer address
        
        // Consume nonce
        self.consume_nonce(&addr_lower);
        
        // Look up or create player
        let player_id = self.get_or_create_player(&addr_lower);
        
        // Create session
        Ok(self.create_session(&addr_lower, player_id))
    }

    /// Get existing player or create a new one
    fn get_or_create_player(&mut self, _address: &str) -> u64 {
        // In production, this would query the database
        // For now, generate a new ID
        let player_id = self.next_player_id;
        self.next_player_id += 1;
        player_id
    }

    /// Clean up expired nonces and sessions
    pub fn cleanup(&mut self) {
        let now = Self::now_secs();
        self.nonces.retain(|_, v| v.expires_at > now);
        self.sessions.retain(|_, v| v.expires_at > now);
    }

    /// Get current time in seconds
    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for WalletAuth {
    fn default() -> Self {
        Self::new()
    }
}

/// Hex encoding helper (simplified)
pub mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_nonce() {
        let mut auth = WalletAuth::new();
        let nonce = auth.generate_nonce("0x123");
        assert_eq!(nonce.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_nonce_uniqueness() {
        let mut auth = WalletAuth::new();
        let nonce1 = auth.generate_nonce("0x123");
        let nonce2 = auth.generate_nonce("0x123");
        assert_ne!(nonce1, nonce2);
    }

    #[test]
    fn test_verify_nonce_valid() {
        let mut auth = WalletAuth::new();
        let nonce = auth.generate_nonce("0x123");
        assert!(auth.verify_nonce("0x123", &nonce));
    }

    #[test]
    fn test_verify_nonce_invalid() {
        let mut auth = WalletAuth::new();
        auth.generate_nonce("0x123");
        assert!(!auth.verify_nonce("0x123", "invalid_nonce"));
    }

    #[test]
    fn test_nonce_consumed_after_use() {
        let mut auth = WalletAuth::new();
        let nonce = auth.generate_nonce("0x123");
        auth.consume_nonce("0x123");
        assert!(!auth.verify_nonce("0x123", &nonce));
    }

    #[test]
    fn test_create_session() {
        let mut auth = WalletAuth::new();
        let session = auth.create_session("0x123", 1);
        assert_eq!(session.player_id, 1);
        assert!(!session.jwt.is_empty());
    }

    #[test]
    fn test_verify_session_valid() {
        let mut auth = WalletAuth::new();
        let session = auth.create_session("0x123", 1);
        assert!(auth.verify_session("0x123", &session.jwt).is_some());
    }

    #[test]
    fn test_verify_session_invalid() {
        let mut auth = WalletAuth::new();
        auth.create_session("0x123", 1);
        assert!(auth.verify_session("0x123", "invalid_jwt").is_none());
    }

    #[test]
    fn test_handle_login_success() {
        let mut auth = WalletAuth::new();
        let address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb";
        let nonce = auth.generate_nonce(address);
        
        // Mock signature verification (returns the same address)
        let session = auth.handle_login(address, &nonce, "mock_sig");
        assert!(session.is_ok());
        let session = session.unwrap();
        assert_eq!(session.address, address.to_lowercase());
    }

    #[test]
    fn test_handle_login_invalid_nonce() {
        let mut auth = WalletAuth::new();
        let result = auth.handle_login("0x123", "bad_nonce", "sig");
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired() {
        let mut auth = WalletAuth::new();
        auth.generate_nonce("0x123");
        auth.cleanup();
        // Nonce should still be valid (not expired)
        assert_eq!(auth.nonces.len(), 1);
    }
}
