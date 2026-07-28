# Spec 008: Teleport Mechanic

> **Objective:** Implement teleport system to navigate large distances quickly

## Problem Statement

Walking across the large hex grid is slow. Players need a fast-travel system to traverse long distances.

## Proposed Solution

- Click hex on minimap/global map to teleport
- Costs 100 Gold per teleport
- Instant teleportation
- Cooldown: 1 minute

## Requirements

### Functional Requirements
1. FR1: Click hex on map to select destination
2. FR2: Calculate teleport cost (100G)
3. FR3: Validate player has enough gold
4. FR4: Execute teleport instantly
5. FR5: Cooldown timer display
6. FR6: Teleport animation/particle effect

### Non-Functional Requirements
1. NFR1: Server-authoritative teleport
2. NFR2: Network-synced teleport position
3. NFR3: Prevent teleport during combat (if implemented)

## Design

### Teleport System
```rust
struct TeleportSystem {
    last_teleport: Option<Instant>,
    cooldown: Duration,
}

impl TeleportSystem {
    fn can_teleport(&self) -> bool {
        match self.last_teleport {
            Some(time) => time.elapsed() >= self.cooldown,
            None => true,
        }
    }
    
    fn execute_teleport(&mut self, player: &mut Player, target_hex: HexCoord) -> Result<()> {
        if !self.can_teleport() {
            return Err(TeleportError::OnCooldown);
        }
        
        if player.gold < 100 {
            return Err(TeleportError::InsufficientGold);
        }
        
        player.gold -= 100;
        player.position = target_hex.to_pixel();
        player.current_hex = target_hex;
        self.last_teleport = Some(Instant::now());
        
        Ok(())
    }
}
```

### UI Integration
```rust
struct TeleportUI {
    selected_hex: Option<HexCoord>,
    destination_hex: Option<HexCoord>,
    cooldown_timer: f32,
    gold_available: u64,
}

fn on_hex_clicked(&mut self, hex: HexCoord) {
    match self.selected_hex {
        Some(_) => {
            self.destination_hex = Some(hex);
        }
        None => {
            self.selected_hex = Some(hex);
        }
    }
}

fn confirm_teleport(&mut self, teleport_system: &mut TeleportSystem) {
    if let Some(dest) = self.destination_hex {
        if teleport_system.can_teleport() && self.gold_available >= 100 {
            teleport_system.execute_teleport(&mut self.player, dest);
            self.selected_hex = None;
            self.destination_hex = None;
        }
    }
}
```

## Acceptance Criteria
- [ ] Click hex to select destination
- [ ] 100G cost deducted correctly
- [ ] Player teleports instantly
- [ ] Cooldown timer displays correctly
- [ ] Cannot teleport on cooldown
- [ ] Teleport animation plays

## Risks
- R1: Teleport spamming (mitigated by cooldown)
- R2: Abuse for griefing (need server validation)

## Open Questions
- Q1: Should longer cooldowns apply during combat?
- Q2: Premium currency discount on teleport cost?
