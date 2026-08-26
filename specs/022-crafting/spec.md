# Spec 022: Grass Regrowth, Logs, Craft Bench & Discovery Crafting

> **Objective:** Close the resource loop — grass replants and regrows, log entities spawn
> in the wild and fund the first craft bench, wood/stone become tools through
> Minecraft-style discovery crafting, and every interaction lands on the selected ground slot.

## Problem Statement

Gathering is currently extract-only: grass/rocks/trees disappear and never come back except
by server re-rolls. Wood and stone have no use. There are no tools and no crafting. The
player also needs a guarantee that any action (plant, harvest, tool use) applies to exactly
the plot they targeted with the selector.

## Design

### 1. Grass regrowth (replantable resource)
- With **Grass** selected in the hotbar, pressing **E** on an empty plot plants it:
  a `Grass` world object is created at that slot's center with `mature_at = now + 75s`.
- Immature grass renders as a small tinted sprout (existing sapling-style path).
- Gathering immature grass is rejected with a "still growing" message (same as trees).
- Gathering mature grass drops Grass (+ the existing 55% seed chance) and frees the slot.
- Growth time: `GRASS_GROWTH_SECS = 75`.

### 2. Logs (new collectible entity)
- New natural world object kind **`Log`** — a fallen log that spawns around the map exactly
  like grass tufts (deterministic roller, same spacing/slot rules, forested terrains).
- Gathering a Log drops **1-2 Log items** and frees the slot (10-60 min respawn via the
  standard tombstone flow).
- Logs are the bootstrap resource: **the first craft bench is built from logs**.
- Trees keep dropping `Wood` (planks) as before; Wood stays a crafting ingredient.

### 3. Craft bench (placeable crafting station)
- Placed with **E** on an empty plot of an adjacent hex while carrying **4+ Logs**: the
  reducer consumes **4 Logs** and creates a `CraftBench` world object at that slot's
  center. No bench item exists — placing IS building the bench (this is the only recipe
  that works without a bench).
- The bench renders as the Tiny Dungeon crate sprite, taller than plants.
- Pressing **E** on the bench's slot opens the craft menu instead of gathering.

### 4. Discovery crafting (no recipe menu)
- The craft menu is a 2×2 grid. The player fills the four cells with ingredient items
  (Wood / Log / Stone / Grass — cycled by clicking a cell, limited by inventory counts)
  and hits **Craft**.
- The server matches the multiset of exactly 4 ingredients against a fixed recipe table;
  unknown combinations fail with "nothing happened" (no hints). Recipes are never listed
  anywhere in the UI.
- **Logs substitute Wood**: the matcher normalizes `Log → Wood` before comparing, so any
  recipe needing wood accepts a mix of Wood and Log.
- Recipe table (order-insensitive, after normalization):
  | Ingredients            | Result |
  |------------------------|------------|
  | Stone, Stone, Stone, Wood | Pickaxe |
  | Stone, Stone, Wood, Wood  | Axe     |
  | Stone, Stone, Grass, Wood | Shovel  |
  | Wood, Wood, Grass, Grass  | Hoe     |
- Crafting requires the target plot to hold a CraftBench (the bench is consumed as a
  station, not as an ingredient — it stays).
- Success consumes the 4 ingredients and adds the result tool to the inventory.

### 5. Selected-plot invariant (tools)
- With a tool selected in the hotbar, **E** performs the tool's action on the **selected
  slot** (nearest matching node in the slot's hex):
  - **Pickaxe** → mine a Rock
  - **Axe** → harvest a Tree (mature)
  - **Shovel** → dig up a Grass tuft
  - **Hoe** → till: plant Wheat on the empty plot (consumes a Seed, reuses the crop system)
- Without a tool, the existing priority stands (gather node → harvest crop → clean →
  plant by held item: Grass / bench placement / Seed).

## Non-Goals
- No recipe persistence across sessions (discovery is per-session memory).
- No tool durability/tiers.
- No new sprites (bench reuses the Dungeon crate tile; log reuses a Tiny Farm wood tile;
  tools are inventory icons only).

## Acceptance Criteria
1. Planting grass on an empty plot creates a growing node visible at the slot center.
2. Gathering before `mature_at` fails; after it succeeds and frees the slot.
3. Log entities spawn in forested terrains and drop Log items when gathered.
4. Placing a bench on an empty plot with ≥4 Logs consumes exactly 4 Logs and renders.
5. Crafting a known recipe at the bench consumes ingredients (Log ≡ Wood) and grants the
   tool; unknown combinations consume nothing and reveal nothing.
6. Tool + E always applies to the selected slot's contents, never another hex.
