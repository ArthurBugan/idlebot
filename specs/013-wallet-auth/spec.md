# Spec 013: Wallet Authentication

> **Objective:** Implement Polygon wallet signature-based login system

## Problem Statement

Players need secure, passwordless authentication using their Polygon wallet. Wallet signature provides proof of ownership without storing credentials.

## Proposed Solution

- Player connects wallet (MetaMask, Rabby, etc.)
- Server generates random nonce
- Player signs nonce with wallet
- Server verifies signature and creates/authenticates session
- JWT or session token for subsequent requests

## Requirements

### Functional Requirements
1. FR1: Connect wallet (MetaMask, Rabby, etc.)
2. FR2: Generate random nonce on login request
3. FR3: Verify wallet signature
4. FR4: Create new player on first login
5. FR5: Return session token/JWT
6. FR6: Handle wallet disconnection
7. FR7: Display connected wallet address in UI

### Non-Functional Requirements
1. NFR1: Signature verification in < 100ms
2. NFR2: No password storage
3. NFR3: Support multiple wallet providers
4. NFR4: Secure nonce generation (CSPRNG)

## Design

### Authentication Flow
```
1. Client: request_login() → Server
2. Server: generate_nonce() → { nonce: "abc123..." }
3. Client: wallet.signMessage(nonce) → { signature: "0x..." }
4. Client: submit_login(nonce, signature) → Server
5. Server: recover_address(signature, nonce) → { address }
6. Server: create_session(address) → { token: "jwt..." }
7. Client: store token, make authenticated requests
```

### Signature Verification
```rust
use alloy::primitives::{Address, Signature, SignedMessage};
use alloy::signers::local::PrivateKeySigner;

fn verify_signature(nonce: &str, signature: &str) -> Result<Address> {
    // Parse signature
    let sig = Signature::from_str(signature)?;
    
    // Recover signer address from signed message
    let message = format!("\x19Ethereum Signed Message:\n{}{}", nonce.len(), nonce);
    let recovered = sig.recover_address_from_msg(message)?;
    
    // Verify recovered address matches expected format
    Ok(recovered)
}
```

### Nonce Generation
```rust
use rand::Rng;

fn generate_nonce() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}
```

### Session Management
```rust
struct Session {
    address: Address,
    created_at: Instant,
    expires_at: Instant,
    player_id: UUID,
}

impl Session {
    fn is_valid(&self) -> bool {
        self.expires_at > Instant::now()
    }
    
    fn generate_jwt(&self) -> Result<String> {
        // Create JWT with address and expiration
        let claims = Claims {
            sub: self.address.to_string(),
            exp: self.expires_at.unix_timestamp() as u64,
            player_id: self.player_id,
        };
        jwt::encode(&claims)
    }
}
```

### SpacetimeDB Integration
```rust
// Server module for auth
pub fn verify_login(
    db: &spacetimedb::DatabaseIndex,
    nonce: &str,
    signature: &str,
) -> Result<spacetimedb::response::Authentication> {
    let address = verify_signature(nonce, signature)?;
    
    // Check if player exists
    let player = db.get_player(&address)?;
    
    match player {
        Some(p) => Ok(spacetimedb::response::Authentication::AlreadyAuthenticated(p)),
        None => {
            // Create new player
            let new_player = db.create_player(&address)?;
            Ok(spacetimedb::response::Authentication::NewPlayer(new_player))
        }
    }
}
```

## Acceptance Criteria
- [ ] Wallet connection works (MetaMask test)
- [ ] Nonce generation secure (32 bytes)
- [ ] Signature verification correct
- [ ] First login creates player
- [ ] Session token returned correctly
- [ ] Wallet disconnection handled
- [ ] UI shows connected address

## Risks
- R1: Phishing attacks (show domain name to user)
- R2: Wallet provider changes API
- R3: Signature replay attacks (use nonce)

## Open Questions
- Q1: Should there be a "remember me" option?
- Q2: Session expiration time?
- Q3: Multi-wallet support per player?
