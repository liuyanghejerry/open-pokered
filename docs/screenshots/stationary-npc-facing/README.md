# Stationary NPC facing

Original source: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c,
engine/overworld/movement.asm UpdateNPCSprite, TryWalking and
CheckSpriteAvailability; object records in data/maps/objects/. STAY selects a
direction but cannot translate. NONE/ANY_DIR permit all four directions; authored
fixed directions and axis restrictions remain. Direction and next delay use two
random draws. A zero delay wraps through 256 update calls. Moving players prevent
a ready turn while existing delays continue; hidden/off-screen sprites do not tick.

253 range values in 122 map JSONs were checked individually against original
STAY NONE (251) / STAY ANY_DIR (2) declarations. Positions, IDs, movement kinds,
initial facing and all text are unchanged. The dataset includes 110 directional
sprites and 143 single-image objects; the latter retain their static image.
Full per-object source records and sprite sizes are in source-data-verification.json.

NPC load paths preserve fixed-facing restrictions separately from current pose.
Blocked text/Delay/healing stages freeze autonomous turns. Scripted NPCs and the
connected remote player keep their movement/facing owner. Existing guide, healer,
link-presence and replay regressions passed. Pure helper tests compare 64 actual
original routine probes (stationary_facing_golden.json); a scene regression checks
turning without displacement and hidden-state freeze.

Before: master ac8dfa2ab6fdba052b03df0a10650b1eeaa01936. After: this branch.
Both capture_stationary_npc_670 fixtures start at ViridianPokecenter (3,3), seed0,
with identical initial screenshots and 900 no-input updates. Each sequence was
recorded twice; all PNGs and manifest bytes repeat exactly. Comparisons use t=400
on both sides, without resampling. Verification JSON includes state and pixel bounds.

Core 2,804; data 263; agent semantics54; all41 app test executables passed.
These tests prove direction rules and the missing autonomous-facing behavior.
Full original hardware cadence, publication latency and RNG trajectory remain
under audit. This PR keeps the existing field driver; it does not claim all original
NPC frames or timing match.

Stationary updates now use the engine's callback in the existing movement pass,
without a second NPC traversal. dotzuki#87 merged this optional generic hook;
old engine APIs keep their no-op stationary behavior. All2,804 core tests and
all41 app executables passed after integration. All900 recorded rows and PNGs
still equal the original candidate bytes. Failed IWRAM scan placement was removed.
Performance and memory thresholds remain unchanged; latest GBA CI gates merging.
