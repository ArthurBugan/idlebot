//! Spec 024 Phase D1 — progression for the built realm:
//! 1. `transfer_building_xp`: the owner sinks their own spare player XP into
//!    a defense structure (player level never demotes; the mid-level ring is
//!    what gets spent).
//! 2. Storage chests (`StorageChest` structures): deposit / withdraw stacks,
//!    and refund contents when the chest dies so a raid loss stays bounded.

use spacetimedb::{reducer, ReducerContext, Table};
use idlecore_core::defense_config as cfg;

use crate::interactions::Outcome;
use crate::objects::{add_item, count_item, remove_item};
use crate::types::{chest_item, defense_structure, player, ChestItem};

/// A chest holds at most 64 distinct item stacks (one row per item kind).
pub const CHEST_CAPACITY_ROWS: u64 = 64;

/// Pure: the level a destroyed structure keeps while its capital survives —
/// half, floored at 1 (Spec 024 §2.2 "level never below 1"). Implausible
/// levels clamp to the level cap first so a corrupt column can't respawn
/// an over-tiered tower.
pub fn half_level(level: u8) -> u8 {
    let level = level.max(1).min(cfg::STRUCTURE_LEVEL_MAX);
    (level / 2).max(1)
}

/// Pure guard for a chest deposit: positive stack and free row capacity.
pub fn deposit_ok(existing_rows: u64, count: u64) -> bool {
    count > 0 && existing_rows < CHEST_CAPACITY_ROWS
}

/// Move every stored stack of one chest back into the owner's inventory.
/// Called when the chest is destroyed (raid or player removal) — the loss
/// is bounded to the chest build cost, never the contents.
pub fn refund_chest(ctx: &ReducerContext, structure_id: u64, owner: &str) {
    let rows: Vec<String> = ctx
        .db
        .chest_item()
        .chest_by_structure()
        .filter(structure_id)
        .map(|r| r.key.clone())
        .collect();
    for key in rows {
        if let Some(row) = ctx.db.chest_item().key().find(key.clone()) {
            add_item(ctx, owner, &row.item, row.count);
            ctx.db.chest_item().key().delete(key);
        }
    }
}

/// Owner-only spend of spare player XP into one structure. Returns the
/// message the HUD toast shows.
pub fn transfer_into(
    ctx: &ReducerContext,
    address: &str,
    structure_id: u64,
) -> Result<String, String> {
    let mut p = crate::economy::find_player(ctx, address)
        .ok_or_else(|| "Player not found".to_string())?;
    let mut s = ctx
        .db
        .defense_structure()
        .structure_id()
        .find(structure_id)
        .ok_or_else(|| "Structure not found".to_string())?;
    if s.owner != p.address {
        return Err("You can only invest XP in your own structures".to_string());
    }
    let with_level = s.level;
    if with_level >= cfg::STRUCTURE_LEVEL_MAX {
        return Err(format!(
            "{} is already max level ({})",
            s.kind,
            cfg::STRUCTURE_LEVEL_MAX
        ));
    }
    // Spendable = the player's surplus into the CURRENT level ring —
    // `level_progress` re-derives the level on read, so spending exactly the
    // surplus can never demote the player.
    let spendable = idlecore_core::progression::level_progress(p.total_xp).1;
    if spendable == 0 {
        return Err("No spare XP in your current level — earn more first".to_string());
    }
    let (new_level, bucket, spent) = cfg::structure_transfer_xp(
        with_level,
        s.total_xp,
        spendable,
    );
    if spent == 0 {
        return Err("Structure is already max level".to_string());
    }
    p.total_xp -= spent;
    // Damage stays; repair/capped-wave fills against the new max.
    let old_level = new_level.max(with_level);
    if let Some(kind) = cfg::defense_kind(&s.kind) {
        s.hp = s.hp.min(cfg::structure_max_hp(kind, new_level));
    }
    s.total_xp = bucket;
    let leveled = new_level > with_level;
    s.level = new_level;
    ctx.db.defense_structure().structure_id().update(s);
    ctx.db.player().address().update(p);
    tracing::info!(
        "TRANSFER-XP {address} → #{structure_id} level {old_level}→{new_level} (+{spent} xp)"
    );
    if leveled {
        Ok(format!("Structure leveled up to {new_level}!"))
    } else {
        Ok(format!("{}", bucket))
    }
}

#[reducer]
pub fn transfer_building_xp(ctx: &ReducerContext, structure_id: u64) -> Result<(), String> {
    let address = crate::player::address_of_identity(ctx, &ctx.sender().to_string())
        .ok_or("Not logged in — connect first".to_string())?;
    transfer_into(ctx, &address, structure_id).map(|msg| {
        crate::player::log_outcome(
            ctx,
            &address,
            "transfer_building_xp",
            Outcome::Ok(msg.clone()),
        );
        tracing::info!("TRANSFER-BUILDING-XP {address} #{structure_id}: {msg}");
    })
}

/// Deposit `count` of `item` from the player's inventory into a chest.
pub fn deposit_into(
    ctx: &ReducerContext,
    address: &str,
    chest_id: u64,
    item: &str,
    count: u64,
) -> Result<(), String> {
    if count == 0 {
        return Err("Deposit at least one item".to_string());
    }
    let chest = ctx
        .db
        .defense_structure()
        .structure_id()
        .find(chest_id)
        .ok_or_else(|| "Chest not found".to_string())?;
    if chest.owner != address {
        return Err("Only the owner stores items here".to_string());
    }
    if chest.kind != cfg::ITEM_STORAGE_CHEST {
        return Err("That structure is not a chest".to_string());
    }
    let existing = ctx
        .db
        .chest_item()
        .chest_by_structure()
        .filter(chest_id)
        .count() as u64;
    if !deposit_ok(existing, count) {
        return Err(format!("Chest is full ({} stacks)", CHEST_CAPACITY_ROWS));
    }
    if count_item(ctx, address, item) < count {
        return Err("Not enough items in your inventory".to_string());
    }
    if !remove_item(ctx, address, item, count) {
        return Err("Not enough items in your inventory".to_string());
    }
    let key = format!("{chest_id}|{item}");
    match ctx.db.chest_item().key().find(key.clone()) {
        Some(mut row) => {
            row.count += count;
            ctx.db.chest_item().key().update(row);
        }
        None => {
            ctx.db.chest_item().insert(ChestItem {
                key,
                structure_id: chest_id,
                item: item.to_string(),
                count,
            });
        }
    }
    Ok(())
}

/// Withdraw `count` of `item` from a chest back into the player inventory.
pub fn withdraw_from(
    ctx: &ReducerContext,
    address: &str,
    chest_id: u64,
    item: &str,
    count: u64,
) -> Result<(), String> {
    if count == 0 {
        return Err("Withdraw at least one item".to_string());
    }
    let chest = ctx
        .db
        .defense_structure()
        .structure_id()
        .find(chest_id)
        .ok_or_else(|| "Chest not found".to_string())?;
    if chest.owner != address {
        return Err("Only the owner withdraws from a chest".to_string());
    }
    let key = format!("{chest_id}|{item}");
    let Some(mut row) = ctx.db.chest_item().key().find(key.clone()) else {
        return Err("The chest does not hold that item".to_string());
    };
    if row.count < count {
        return Err("Chest holds fewer items than requested".to_string());
    }
    row.count -= count;
    if row.count == 0 {
        ctx.db.chest_item().key().delete(key);
    } else {
        ctx.db.chest_item().key().update(row);
    }
    add_item(ctx, address, item, count);
    Ok(())
}

#[reducer]
pub fn chest_deposit(
    ctx: &ReducerContext,
    chest_id: u64,
    item: String,
    count: u64,
) -> Result<(), String> {
    let address = crate::player::address_of_identity(ctx, &ctx.sender().to_string())
        .ok_or("Not logged in — connect first".to_string())?;
    deposit_into(ctx, &address, chest_id, &item, count)?;
    crate::player::log_outcome(
        ctx,
        &address,
        "chest_deposit",
        Outcome::ok(format!("stored {count}×{item} in chest #{chest_id}")),
    );
    Ok(())
}

#[reducer]
pub fn chest_withdraw(
    ctx: &ReducerContext,
    chest_id: u64,
    item: String,
    count: u64,
) -> Result<(), String> {
    let address = crate::player::address_of_identity(ctx, &ctx.sender().to_string())
        .ok_or("Not logged in — connect first".to_string())?;
    withdraw_from(ctx, &address, chest_id, &item, count)?;
    crate::player::log_outcome(
        ctx,
        &address,
        "chest_withdraw",
        Outcome::ok(format!("took {count}×{item} from chest #{chest_id}")),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_level_never_dips_below_one() {
        assert_eq!(half_level(0), 1, "legacy rows respawn at L1");
        assert_eq!(half_level(1), 1);
        assert_eq!(half_level(2), 1);
        assert_eq!(half_level(3), 1); // 3/2 = 1 (integer floor)
        assert_eq!(half_level(4), 2);
        assert_eq!(half_level(5), 2);
        assert_eq!(half_level(255), 2, "over-cap garbage clamps to the top then halves");
    }

    #[test]
    fn deposit_guard_blocks_zero_and_full_chests() {
        assert!(!deposit_ok(0, 0), "zero stacks rejected");
        assert!(deposit_ok(0, 1));
        assert!(deposit_ok(CHEST_CAPACITY_ROWS - 1, 100));
        assert!(!deposit_ok(CHEST_CAPACITY_ROWS, 5), "64 rows = full");
        assert!(!deposit_ok(CHEST_CAPACITY_ROWS + 3, 5));
    }
}
