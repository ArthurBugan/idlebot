//! UI system -- HUD overlay, interaction buttons, vehicle menu, teleport UI.
//!
//! Uses console-based output for local single-player testing.
//! In production, would use Bevy UI (Button, Text, Image nodes).

use crate::economy;
use crate::teleport;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// HUD Display Data
// ---------------------------------------------------------------------------

/// Player status display (rendered as text in HUD)
#[derive(Debug, Clone)]
pub struct HuddData {
    pub gold: u64,
    pub xp: u64,
    pub level: u32,
    pub eco_points: u64,
    pub player_name: String,
    pub vehicle: String,
    pub current_hex: u64,
    pub hex_terrain: String,
    pub players_nearby: usize,
    pub actions_available: Vec<String>,
    pub cooldown_remaining: u64,
}

impl HuddData {
    pub fn render(&self) -> String {
        let mut out = String::new();

        // Top bar
        out.push_str(&format!(
            "\n\x1b[1m[ {} ]\x1b[0m\n",
            self.player_name
        ));

        // Status line
        let _vehicle_tag = if self.vehicle.is_empty() {
            "(no vehicle)"
        } else {
            &format!("({})", self.vehicle)
        };
        out.push_str(&format!(
            "  Lvl {} | Gold: {}G | XP: {}/{}\x1b[0m\n",
            self.level, self.gold, self.xp, self.xp_for_level(),
        ));

        // Eco / hex info
        out.push_str(&format!(
            "  Eco: {} | Hex: {} | Terrain: {} | Players: {}/{}\x1b[0m\n\n",
            self.eco_points,
            self.current_hex,
            self.hex_terrain,
            self.players_nearby,
            economy::MAX_HEX_PLAYERS,
        ));

        // Action buttons
        for action in &self.actions_available {
            out.push_str(&format!("  [{}] {}\n", self.render_action_icon(action), action));
        }

        // Cooldown indicator
        if self.cooldown_remaining > 0 {
            let mins = self.cooldown_remaining / 60;
            let secs = self.cooldown_remaining % 60;
            let time_str = if mins > 0 {
                format!("{}m {:02}s", mins, secs)
            } else {
                format!("{}s", secs)
            };
            out.push_str(&format!("  \x1b[1;33mCooldown: {} remaining\x1b[0m\n\n",
                time_str));
        }

        out
    }

    /// Generate full output
    pub fn render_full(&self) -> String {
        let hud = self.render();
        let teleport_cost_str = teleport::get_teleport_cost_display(self.level);
        let vehicle_menu = self.vehicle_menu();
        let marketplace_preview = self.marketplace_preview();

        let mut out = String::new();
        out.push_str(&hud);
        out.push_str(&format!("  {} | {}", teleport_cost_str, vehicle_menu));
        out.push_str(&marketplace_preview);
        out
    }

    fn render_action_icon(&self, action: &str) -> &'static str {
        match action {
            "Plant" => "\x1b[32m[✓]\x1b[0m",
            "Harvest" => "\x1b[33m[✦]\x1b[0m",
            "Clean" => "\x1b[36m[🧹]\x1b[0m",
            "Clear" => "\x1b[37m[~]\x1b[0m",
            _ => "[ ]",
        }
    }

    fn xp_for_level(&self) -> u64 {
        economy::xp_for_next_level(self.level)
    }

    fn vehicle_menu(&self) -> String {
        let current = if self.vehicle.is_empty() {
            "NONE"
        } else {
            &self.vehicle
        };
        let mut items = Vec::new();

        for v in economy::VEHICLE_DEFINITIONS {
            let name = if v.cost == 0 {
                "None"
            } else {
                current
            };
            if name == current {
                items.push(format!("\x1b[1;36m {} ({}x) \x1b[0m", v.speed_multiplier, name));
            } else {
                items.push(format!(" {} ({}G)  ", v.speed_multiplier, name));
            }
        }

        items.join("\n")
    }

    fn marketplace_preview(&self) -> String {
        let mut items = Vec::new();
        items.push("  MARKET: List (50G) / Buy");
        items.push("  {Templates, Agents, Code}");
        items.join("\n")
    }

}

// ---------------------------------------------------------------------------
// Interaction UI State
// ---------------------------------------------------------------------------

/// State for the interaction buttons (bottom screen area)
#[derive(Debug, Clone)]
pub struct InteractionUi {
    pub available: Vec<String>,
    pub cooldown_active: bool,
    pub selected_action: Option<String>,
}

impl InteractionUi {
    pub fn new() -> Self {
        Self {
            available: vec![
                "Plant (10G, +5 XP)".to_string(),
                "Harvest (+15G, +10 XP)".to_string(),
                "Clean (+20G, +15 XP, +30 Eco)".to_string(),
                "Clear Terrain (15G, +5 XP)".to_string(),
                "Teleport (costs Gold)".to_string(),
            ],
            cooldown_active: false,
            selected_action: None,
        }
    }

    /// Check if action is available (5s cooldown)
    pub fn update_available(gs: &economy::LocalGameState) -> Self {
        let can_act = gs.can_act();
        let _cooldown_secs = if can_act { 0 } else { 5 };
        let cooldown_active = !can_act;

        Self {
            available: if can_act {
                vec![
                    "Plant (10G, +5 XP)".to_string(),
                    "Harvest (+15G, +10 XP)".to_string(),
                    "Clean (+20G, +15 XP, +30 Eco)".to_string(),
                    "Clear Terrain (15G, +5 XP)".to_string(),
                    "Teleport (costs Gold)".to_string(),
                ]
            } else {
                vec!["Action on cooldown (5s)".to_string()]
            },
            cooldown_active,
            selected_action: None,
        }
    }

    /// Render interaction bar
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "\n\x1b[1;34m[ INTERACTION ]\x1b[0m\n"
        ));
        for action in &self.available {
            if self.cooldown_active && action == "Action on cooldown (5s)" {
                out.push_str(&format!("  \x1b[33m⏳ {}\x1b[0m\n", action));
            } else {
                out.push_str(&format!("  \x1b[32m▶ {}\x1b[0m\n", action));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Teleport UI
// ---------------------------------------------------------------------------

/// Teleport UI state
#[derive(Debug, Clone)]
pub struct TeleportUi {
    pub targets: Vec<teleport::TeleportTarget>,
    pub target_selected: Option<(u64, i32)>, // (hex_id, distance)
    pub selected_name: Option<String>,
}

impl TeleportUi {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            target_selected: None,
            selected_name: None,
        }
    }

    /// Populate with nearby hexes
    pub fn populate(gs: &economy::LocalGameState, player_hex: u64) -> Self {
        let targets = teleport::get_teleport_options(gs, player_hex);
        Self {
            targets,
            target_selected: None,
            selected_name: None,
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "\n\x1b[1;35m[ TELEPORT ]\x1b[0m\n"
        ));

        if self.targets.is_empty() {
            out.push_str("  No reachable hexes nearby.\n");
        } else {
            for (i, target) in self.targets.iter().enumerate() {
                let selected = if let Some((tid, _)) = self.target_selected {
                    tid == target.hex_id
                } else {
                    false
                };
                let dist_str = format!("{} hex away", target.distance);
                out.push_str(&format!(
                    "  {} [{}] hex {}: {}\n",
                    if selected { "\x1b[1;35m★\x1b[0m" } else { " " },
                    i,
                    target.hex_id,
                    dist_str,
                ));
            }
        }

        if let Some((_, dist)) = self.target_selected {
            let cost = teleport::calc_teleport_cost(0); // placeholder
            out.push_str(&format!(
                "\n  Target: hex {}, {} away\n  Cost: {}G\n",
                self.targets.iter()
                    .find(|t| t.hex_id == self.target_selected.unwrap().0)
                    .map(|t| t.hex_id.to_string())
                    .unwrap_or("??".to_string()),
                dist,
                cost,
            ));
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Vehicle Purchase Menu
// ---------------------------------------------------------------------------

/// Vehicle purchase menu UI state
#[derive(Debug, Clone)]
pub struct VehicleMenu {
    pub selected_vehicle: Option<(String, u64, f32)>, // (name, cost, speed)
}

impl VehicleMenu {
    pub fn new() -> Self {
        Self { selected_vehicle: None }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "\n\x1b[1;37m[ VEHICLE ]\x1b[0m\n"
        ));

        for v in economy::VEHICLE_DEFINITIONS {
            let name = if v.cost == 0 {
                "(equipped: None)"
            } else {
                &v.speed_multiplier.to_string()
            };
            let emoji = if name == "(equipped: None)" {
                "\x1b[37m(⊘)\x1b[0m"
            } else {
                "\x1b[36m(●)\x1b[0m"
            };
            out.push_str(&format!("  {} {} ({}G) → {}\n",
                emoji, v.speed_multiplier, v.cost, name));
        }

        out
    }

    /// Select a vehicle
    pub fn select_vehicle(&mut self, vehicle_name: &str) -> Option<(String, u64, f32)> {
        for v in economy::VEHICLE_DEFINITIONS {
            let name = if v.cost == 0 {
                "none".to_string()
            } else {
                v.cost.to_string()
            };
            if name == vehicle_name {
                let speed = if v.cost == 0 {
                    v.speed_multiplier
                } else {
                    0.0
                };
                self.selected_vehicle = Some((v.cost.to_string(), v.speed_multiplier as u64, speed));
                println!("[UI] Vehicle selected: {} (cost: {}G)", vehicle_name, v.cost);
                return Some((vehicle_name.to_string(), v.cost, v.speed_multiplier));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Console Rendering (for local testing)
// ---------------------------------------------------------------------------

/// Render full game state to console
pub fn render_console(gs: &economy::LocalGameState) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let _ = now; // suppress warning

    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "\n\x1b[1;42m╔══════════════════════════════════════════════╗\x1b[0m\n║          IDLEBOT -- Local Testing Mode       ║\n╚══════════════════════════════════════════════╝\n"
    ));

    // Player status
    let _hud = HuddData {
        gold: gs.gold,
        xp: gs.xp,
        level: gs.level,
        eco_points: gs.eco_points,
        player_name: gs.player_address.clone(),
        vehicle: gs.economy.vehicle.clone(),
        current_hex: gs.current_hex_id,
        hex_terrain: gs.current_terrain.clone(),
        players_nearby: gs.nearby_hexes.len(),
        actions_available: Vec::new(),
        cooldown_remaining: 0,
    };

    // Interaction UI
    let interaction = InteractionUi::update_available(gs);
    out.push_str(&interaction.render());

    // HUD data
    out.push_str(&format!(
        "\n\x1b[1;34mHUD:\x1b[0m Gold: {}G | XP: {}/{} | Lvl: {} | Eco: {}\n",
        gs.gold, gs.xp, economy::xp_for_next_level(gs.level), gs.level, gs.eco_points,
    ));

    // Cooldown
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cooldown_secs = now.saturating_sub(gs.last_action_time);
    let cooldown_remaining = if cooldown_secs >= 5 { 0 } else { 5 - cooldown_secs };

    if cooldown_remaining > 0 {
        let remaining = cooldown_remaining;
        let mins = remaining / 60;
        let secs = remaining % 60;
        if mins > 0 {
            out.push_str(&format!(
                "\x1b[33m⏳ Cooldown: {}m {:02}s remaining\x1b[0m\n",
                mins, secs
            ));
        } else {
            out.push_str(&format!(
                "\x1b[33m⏳ Cooldown: {}s remaining\x1b[0m\n",
                secs
            ));
        }
    }

    // Teleport UI
    let player_hex = gs.current_hex_id;
    let _teleport_ui = TeleportUi::populate(gs, player_hex);
    let _teleport_str = teleport::get_teleport_cost_display(gs.level);
    out.push_str(&format!(
        "\n\x1b[1;35m{}\x1b[0m\n",
        teleport::format_teleport_cost(gs.economy.gold),
    ));

    // Debug commands
    out.push_str(&format!(
        "\n\x1b[1;31m[ DEBUG COMMANDS ]\x1b[0m\n  /add_gold <amount>  -- Add gold\n  /teleport <hex_id>  -- Teleport to hex\n  /teleport          -- Show nearby hexes\n  /menu              -- Show all options\n  /reset             -- Reset game state\n"
    ));

    out.push_str(&format!(
        "\n\x1b[1;36m───────────────────────────────────────────────\x1b[0m\n"
    ));

    println!("{}", out);
}

/// Print quick status update
pub fn print_status(gs: &economy::LocalGameState) {
    println!("\n[STATUS] {} | Gold: {}G | XP: {} | Lvl: {} | Eco: {} | Hex: {}",
        gs.player_address, gs.gold, gs.xp, gs.level, gs.eco_points, gs.current_hex_id);
}
