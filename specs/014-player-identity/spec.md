# Spec 014: Player Identity Management

> **Objective:** Implement player identity system with wallet-linked accounts and player data

## Problem Statement

Players need persistent identity across sessions. Their wallet address serves as the unique identifier, with profile data stored server-side.

## Proposed Solution

- Wallet address as unique player ID
- Player profile stored in SpacetimeDB
- Support for display name, avatar, bio
- Activity tracking and statistics

## Requirements

### Functional Requirements
1. FR1: Player ID = wallet address (64-char hex)
2. FR2: Store player profile data
3. FR3: Display name (optional, up to 20 chars)
4. FR4: Avatar selection (5 default options)
5. FR5: Activity statistics (play time, actions, etc.)
6. FR6: Player search by address or name

### Non-Functional Requirements
1. NFR1: Player data synced to server
2. NFR2: Profile updates don't conflict
3. NFR3: Statistics accurate and up-to-date

## Design

### Player Data Model
```rust
struct Player {
    id: UUID,
    address: String, // wallet address
    display_name: Option<String>,
    avatar: AvatarType,
    bio: Option<String>,
    level: u32,
    total_xp: u64,
    gold: u64,
    eco_points: u32,
    position: (f32, f32),
    current_hex: Option<HexCoord>,
    created_at: Instant,
    last_login: Instant,
    
    // Statistics
    total_play_time: Duration,
    plants_planted: u64,
    plants_harvested: u64,
    pollution_cleaned: u64,
    templates_published: u64,
    templates_purchased: u64,
}

enum AvatarType {
    Tetrahedron,  // Default placeholder
    Cube,
    Sphere,
    Cylinder,
    Cone,
}
```

### Player Management
```rust
struct PlayerManager {
    players: HashMap<UUID, Player>,
}

impl PlayerManager {
    fn create_player(&mut self, address: &str) -> Player {
        let player = Player {
            id: Uuid::new_v4(),
            address: address.to_string(),
            display_name: None,
            avatar: AvatarType::Tetrahedron,
            bio: None,
            level: 1,
            total_xp: 0,
            gold: 100, // Starting gold
            eco_points: 0,
            position: (0.0, 0.0),
            current_hex: None,
            created_at: Instant::now(),
            last_login: Instant::now(),
            total_play_time: Duration::ZERO,
            plants_planted: 0,
            plants_harvested: 0,
            pollution_cleaned: 0,
            templates_published: 0,
            templates_purchased: 0,
        };
        
        self.players.insert(player.id, player);
        player
    }
    
    fn update_display_name(&mut self, player_id: UUID, name: &str) -> Result<()> {
        if name.len() > 20 {
            return Err(PlayerError::NameTooLong);
        }
        
        if name.chars().any(|c| !c.is_alphanumeric()) {
            return Err(PlayerError::InvalidChars);
        }
        
        if let Some(player) = self.players.get_mut(&player_id) {
            player.display_name = Some(name.to_string());
            Ok(())
        } else {
            Err(PlayerError::PlayerNotFound)
        }
    }
    
    fn get_player_stats(&self, player_id: UUID) -> Option<PlayerStats> {
        self.players.get(&player_id).map(|p| PlayerStats {
            level: p.level,
            total_xp: p.total_xp,
            plants_planted: p.plants_planted,
            plants_harvested: p.plants_harvested,
            pollution_cleaned: p.pollution_cleaned,
            templates_published: p.templates_published,
            templates_purchased: p.templates_purchased,
            play_time: p.total_play_time,
        })
    }
}
```

### SpacetimeDB Tables
```sql
CREATE TABLE players (
    id UUID PRIMARY KEY,
    address TEXT NOT NULL UNIQUE,
    display_name TEXT,
    avatar TEXT DEFAULT 'tetrahedron',
    bio TEXT,
    level INT DEFAULT 1,
    total_xp BIGINT DEFAULT 0,
    gold BIGINT DEFAULT 100,
    eco_points INT DEFAULT 0,
    position_x REAL DEFAULT 0.0,
    position_y REAL DEFAULT 0.0,
    current_hex_q INTEGER,
    current_hex_r INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_login TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_players_address ON players(address);
CREATE INDEX idx_players_display_name ON players(display_name);
```

## Acceptance Criteria
- [ ] Player created on first login
- [ ] Display name can be set/updated
- [ ] Avatar selection works
- [ ] Statistics tracked correctly
- [ ] Player search works
- [ ] Data persists across sessions

## Risks
- R1: Address uniqueness (hash collisions?)
- R2: Display name conflicts
- R3: Large player database performance

## Open Questions
- Q1: Should players have player tags/roles?
- Q2: Admin tools for player management?
- Q3: Player reputation system?
