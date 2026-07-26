//! Sistema de Progressão — Níveis & Unlocks (client-side)

/// Calcular nível baseado em XP acumulado
pub fn calculate_level(xp: u64) -> u32 {
    let mut level = 1u32;
    let mut remaining = xp;

    while remaining >= xp_for_next_level(level) {
        remaining -= xp_for_next_level(level);
        level += 1;
    }

    level
}

/// XP necessário para o próximo nível (100 * L²)
pub fn xp_for_next_level(level: u32) -> u64 {
    level as u64 * 100 * level as u64
}

/// XP necessário para alcançar um nível específico
pub fn xp_needed_for(level: u32) -> u64 {
    let mut total = 0u64;
    for l in 1..level {
        total += xp_for_next_level(l);
    }
    total
}
