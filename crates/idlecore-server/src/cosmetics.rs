//! Cosmetics (Spec 007) — hats/auras/trails purchased with gold or USDT,
//! eco-point unlocks (Spec 020 FR5), equip/unequip.

use spacetimedb::{ReducerContext, Table};
use crate::economy::{add_usdt, spend_gold, spend_usdt};
use crate::types::{now_secs, player, player_cosmetic, CosmeticOwned};

/// Catalog (Spec 007 table): category, tier, gold cost, USDT cost (6-dec).
pub fn catalog() -> [(&'static str, &'static str, u64, u64); 6] {
    [
        ("Hat", "Basic", 200, 0),
        ("Hat", "Premium", 0, 1_000_000), // 1.0 USDT
        ("Aura", "Basic", 500, 0),
        ("Aura", "Premium", 0, 2_500_000), // 2.5 USDT
        ("Trail", "Basic", 300, 0),
        ("Trail", "Premium", 0, 1_500_000), // 1.5 USDT
    ]
}

/// Eco-point unlock gate (Spec 020 FR5): 500 EP unlocks the "Eco Warrior" set.
pub const ECO_WARRIOR_UNLOCK_EP: u32 = 500;

/// Spec 007 FR1-FR3 + FR6: purchase a cosmetic (gold or USDT).
pub fn buy_cosmetic(
    ctx: &ReducerContext,
    address: &str,
    category: &str,
    tier: &str,
) -> Result<String, String> {
    let Some(&(cat, t, gold_cost, usdt_cost)) = catalog().iter().find(|(c, t, _, _)| *c == category && *t == tier)
    else {
        return Err("Unknown cosmetic".to_string());
    };

    let mut p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    // Eco unlock: Eco Warrior hat requires 500 EP.
    if category == "Hat" && tier == "Basic" && p.eco_points < ECO_WARRIOR_UNLOCK_EP {
        return Err(format!(
            "Eco Warrior hat requires {} eco points (you have {})",
            ECO_WARRIOR_UNLOCK_EP, p.eco_points
        ));
    }

    let owned = ctx
        .db
        .player_cosmetic()
        .iter()
        .any(|c| c.player == p.address && c.category == cat && c.tier == t);
    if owned {
        return Err("Cosmetic already owned".to_string());
    }

    if gold_cost > 0 {
        spend_gold(ctx, &mut p, gold_cost, "buy_cosmetic")?;
    } else if usdt_cost > 0 {
        spend_usdt(ctx, &mut p, usdt_cost, "buy_cosmetic")?;
    }

    let cid = ctx.db.player_cosmetic().insert(CosmeticOwned {
        cosmetic_id: 0,
        player: p.address.clone(),
        category: cat.to_string(),
        tier: t.to_string(),
        purchased_at: now_secs(ctx),
        equipped: false,
    });

    let cid = cid.cosmetic_id;
    tracing::info!("COSMETIC: {} bought {cat}/{t} (id {})", address, cid);
    Ok(format!("Purchased {cat} ({t})"))
}

/// Spec 007 FR4: equip a cosmetic; auto-unequips the previous one in the
/// same category.
pub fn equip_cosmetic(ctx: &ReducerContext, address: &str, cosmetic_id: u32) -> Result<(), String> {
    let p = crate::economy::find_player(ctx, &address.to_lowercase())
        .ok_or_else(|| "Player not found".to_string())?;

    let Some(mut c) = ctx
        .db
        .player_cosmetic()
        .cosmetic_id()
        .find(cosmetic_id)
    else {
        return Err("Cosmetic not found".to_string());
    };
    if c.player != p.address {
        return Err("Not your cosmetic".to_string());
    }

    for mut other in ctx.db.player_cosmetic().iter() {
        if other.player == p.address && other.category == c.category && other.equipped {
            other.equipped = false;
            ctx.db.player_cosmetic().cosmetic_id().update(other);
        }
    }
    let entry = format!("{}/{}", c.category, c.tier);
    c.equipped = true;
    ctx.db.player_cosmetic().cosmetic_id().update(c);

    let mut equipped: Vec<String> = serde_json::from_str(&p.cosmetics).unwrap_or_default();
    equipped.retain(|e| e != &entry);
    equipped.push(entry);
    let mut p = p;
    p.cosmetics = serde_json::to_string(&equipped).unwrap();
    ctx.db.player().address().update(p);
    Ok(())
}

/// Reward the player's first eco-points (grants spendable USDT? No — this is
/// a hook for future USDT faucets; kept for the marketplace flow).
#[allow(dead_code)]
pub fn grant_faucet_usdt(ctx: &ReducerContext, address: &str, amount: u64) {
    if let Some(mut p) = crate::economy::find_player(ctx, &address.to_lowercase()) {
        if p.usdt == 0 {
            add_usdt(ctx, &mut p, amount, "faucet");
        }
    }
}