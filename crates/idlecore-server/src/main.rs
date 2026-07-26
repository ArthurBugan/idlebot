//! IdleBot Server Module
//!
//! Implementa as tabelas e funções do SpacetimeDB para o jogo

use serde::{Deserialize, Serialize};
use spacetimedb::{entrypoint, init, pubsub, scheduled, table};

/// Tabela de jogadores
#[table(name = "player", public)]
pub struct PlayerEntry {
    /// Endereço wallet (chave primária)
    pub address: String,
    /// Posição x
    pub position_x: f32,
    /// Posição y
    pub position_y: f32,
    /// Hex ID atual
    pub hex_id: u64,
    /// XP acumulado
    pub xp: u64,
    /// Gold acumulado
    pub gold: u64,
    /// Nível
    pub level: u32,
    /// Eco points
    pub eco_points: u64,
    /// Último login (Unix timestamp)
    pub last_seen: u64,
    /// Status online
    pub is_online: bool,
    /// Tipo de veículo
    pub vehicle: String,
    /// Cosméticos comprados (JSON)
    pub cosmetics: String,
    /// Templates no inventário (JSON)
    pub templates: String,
    /// Limite de templates
    pub templates_limit: u32,
}

/// Tabela de hexágonos
#[table(name = "hex_tile", public)]
pub struct HexTileEntry {
    /// ID do hexágono (q << 32 | r)
    pub hex_id: u64,
    /// Posição x do centro
    pub center_x: f32,
    /// Posição y do centro
    pub center_y: f32,
    /// Tipo de terreno
    pub terrain: String,
    /// Planta atual (JSON, nullable)
    pub plant: Option<String>,
    /// Está poluído?
    pub is_polluted: bool,
    /// Rating eco (0-100)
    pub eco_rating: u32,
}

/// Tabela de channels de voz
#[table(name = "voice_channel", public)]
pub struct VoiceChannelEntry {
    /// Hex ID do canal
    pub hex_id: u64,
    /// Players no canal (JSON array de addresses)
    pub players: String,
    /// Quando o canal foi criado
    pub created_at: u64,
    /// Última atividade
    pub last_activity: u64,
}

/// Tabela de listings do market
#[table(name = "market_listing", public)]
pub struct MarketListingEntry {
    /// ID do listing
    pub listing_id: u64,
    /// Vendedor
    pub seller: String,
    /// Título
    pub title: String,
    /// URL do GitHub
    pub github_url: String,
    /// Descrição
    pub description: String,
    /// Preço em USDT
    pub price_usdt: f64,
    /// Publicado em
    pub published_at: u64,
    /// Vendido?
    pub sold: bool,
}

// ============================================================
// Pub/Sub Events
// ============================================================

/// Quando jogador muda de hex
#[pubsub]
pub fn hex_changed() {}

/// Quando jogador entra num canal de voz
#[pubsub]
pub fn voice_join() {}

/// Quando jogador sai de um canal de voz
#[pubsub]
pub fn voice_leave() {}

/// Quando jogador ganha idle
#[pubsub]
pub fn idle_gained() {}

/// Quando item é comprado
#[pubsub]
pub fn item_purchased() {}

/// Quando listing é criado
#[pubsub]
pub fn listing_created() {}

/// Quando listing é vendido
#[pubsub]
pub fn listing_sold() {}

// ============================================================
// Module
// ============================================================

#[module]
pub mod idlebot_module {
    use super::*;

    /// Inicializar mundo (chamado uma vez no deploy)
    #[init]
    pub fn init() {
        crate::world::generate_initial_world();
        tracing::info!("IdleBot world initialized");
    }

    /// Login / Register
    #[entrypoint]
    pub fn login(wallet_address: String, signature: String, nonce: u64) {
        crate::auth::handle_login(&wallet_address, &signature, nonce);
    }

    /// Sair / Desconectar
    #[entrypoint]
    pub fn logout(wallet_address: String) {
        crate::auth::mark_offline(&wallet_address);
        tracing::info!("Player logged out: {}", wallet_address);
    }

    /// Mover jogador
    #[entrypoint]
    pub fn move_player(wallet_address: String, target_x: f32, target_y: f32) {
        crate::world::move_player(&wallet_address, target_x, target_y);
    }

    /// Teleportar jogador
    #[entrypoint]
    pub fn teleport_player(wallet_address: String, target_hex_id: u64) {
        let cost = 100u64;
        crate::world::teleport_player(&wallet_address, target_hex_id, cost);
    }

    /// Interação com hex (plantar, colher, limpar)
    #[entrypoint]
    pub fn interact_hex(
        wallet_address: String,
        hex_id: u64,
        action: String,
        plant_type: Option<String>,
    ) {
        let result = crate::world::interact_hex(&wallet_address, hex_id, &action, plant_type);
        match result {
            Ok(action_result) => {
                if let crate::world::ActionResult::Success {
                    xp_gained,
                    gold_gained,
                    ..
                } = &action_result
                {
                    if *xp_gained > 0 || *gold_gained > 0 {
                        idle_gained::publish(());
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Action failed: {}", e);
            }
        }
    }

    /// Comprar item (veículo ou cosmético)
    #[entrypoint]
    pub fn buy_item(wallet_address: String, item_type: String, item_name: String, cost: u64) {
        crate::world::buy_item(&wallet_address, &item_type, &item_name, cost);
        item_purchased::publish(());
    }

    /// Publishar template no market
    #[entrypoint]
    pub fn publish_template(
        wallet_address: String,
        title: String,
        github_url: String,
        description: String,
        price_usdt: f64,
    ) {
        crate::market::publish_template(
            &wallet_address,
            title,
            github_url,
            description,
            price_usdt,
        );
        listing_created::publish(());
    }

    /// Comprar template (confirmado via blockchain event)
    #[entrypoint]
    pub fn complete_template_purchase(
        seller: String,
        buyer: String,
        listing_id: u64,
        price_usdt: f64,
    ) {
        crate::market::complete_purchase(&seller, &buyer, listing_id, price_usdt);
        listing_sold::publish(());
    }

    /// Join voice channel
    #[entrypoint]
    pub fn voice_join_hex(wallet_address: String, hex_id: u64) {
        crate::voice::join_channel(&wallet_address, hex_id);
        voice_join::publish(());
    }

    /// Leave voice channel
    #[entrypoint]
    pub fn voice_leave_hex(wallet_address: String, hex_id: u64) {
        crate::voice::leave_channel(&wallet_address, hex_id);
        voice_leave::publish(());
    }

    // ============================================================
    // Scheduled Functions
    // ============================================================

    /// Atualizar crescimento de plantas (a cada 10 segundos)
    #[scheduled(every = 10)]
    pub fn update_plants() {
        crate::world::update_plants();
    }

    /// Calcular idle gains (a cada 5 minutos)
    #[scheduled(every = 300)]
    pub fn calculate_idle() {
        crate::auth::calculate_idle_gains();
        idle_gained::publish(());
    }

    /// Cleanup voice channels inativos (a cada 1 minuto)
    #[scheduled(every = 60)]
    pub fn cleanup_voice_channels() {
        crate::voice::cleanup_inactive_channels();
    }

    /// Cleanup listings antigos não vendidos (a cada 1 hora)
    #[scheduled(every = 3600)]
    pub fn cleanup_old_listings() {
        crate::market::cleanup_old_listings();
    }
}
