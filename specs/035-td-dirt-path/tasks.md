# Spec 035 tasks — dirt path

- [x] T1 core: `hex_line(q0,r0,q1,r1)` cube-lerp pure fn + tests (adjacent
  steps, endpoint+length, determinism, spawn==capital degenerate)
- [x] T2 server: `Capital.spawn_q/spawn_r` additive columns; `place_capital`
  stamps the frontier spawn (walk_out extended to ≥8 hexes)
- [x] T3 server: raid march target = next line hex (`raid_march_target`,
  legacy (0,0) capital falls back to the capital hex); spawn_wave uses the
  single stored spawn point; hunt-chase unaffected; tests pinned
- [x] T4 client: dirt overlays for every path hex + darker spawn pad;
  tracker keyed by capital_id, rebuild only when the row set changes
  (REV: replaced by the floor-painted ribbon — see C3)
- [x] T6 REV playtest 2: capital contact gate dist ≤ 1 (no ranged sniping),
  spawn distance 8 → 5, smooth ribbon lane instead of tile blocks
- [x] T7 REV playtest 3: RANDOM CURVED path. Server: `generate_raid_path` —
  walk out from the capital to a random 6–9 hexes with a random 2–5
  sidestep budget (curves), trace stored on `Capital.path_hexes` (spawn
  first, capital last); `raid_march_target` walks the stored nodes;
  re-placement rolls a fresh trace. Client: ribbon through the stored
  hexes; mid-session placement repaints lane tiles via despawn+respawn.
  Schema change → DB wiped (approved).
- [x] T5 verify: core/server/client tests; publish + binding regen
  (capital_type gains the two fields)
