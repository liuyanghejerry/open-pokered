# GBA memory-peak audit (2026-09-25)

Hardware reality check: an allocation failure on GBA is Rust's alloc-error
panic, which prints only to the mGBA debug channel — on a real cartridge it
is an invisible halt. Every allocation whose size can exceed the largest
free block at its moment is therefore a latent freeze. This audit covers the
whole bare-metal build (`target_os = "none"`).

## Measured heap profile

Probed every frame with a fallible `try_reserve_exact` bisection
(`pokered_app::game::largest_free_block`, gated by `repro-markers`), running
the full `repro-rival` flow (intro → Oak's Lab → rival battle → save).
Values are the **largest allocatable contiguous block**:

| phase | largest free block |
|---|---|
| boot / empty | 65,536 B |
| main menu / Oak speech | 58 KiB → 49 KiB |
| overworld after intro | 40 KiB |
| battle transition (tightest window) | **26,112 B** (was 20,616 B before the boot-footprint fix) |
| after the first rival battle (Oak's Lab) | ~30 KiB |
| save in the lab (after streaming fix) | succeeds |

Heap region: `0x0201_77E8..0x0204_0000` = 166,424 B. agb's block allocator is
a free list with a forward-only bump pointer, so freed blocks do not restore
the "fresh" ceiling — the boot-time allocation order permanently shapes the
largest chunk.

## Fixed in this audit

1. **Dialogue text leaked permanently** (`DialoguePage { line1: &'static str }`
   was `Box::leak`'d at every construction: signs, item messages, NPC
   conversations, script dialogues — ~64-128 B per conversation, unbounded
   over a session; hundreds of conversations equal tens of KiB on a ~30 KiB
   working headroom). The lines are now owned `Box<str>`: same per-dialogue
   allocation, freed when the dialogue drops. Only the one-time battle-rules
   leaks remain (deliberate, bounded).
2. **Boot-time 32 KiB scratch stack** removed: `game_main`'s own frame is
   under a kilobyte (after the `#[inline(never)]` constructor fix), so the
   SRAM import runs on the main stack. At boot this used to keep 32 KiB
   (scratch stack) + 32 KiB (SRAM image) live simultaneously; the smaller
   footprint also moved the battle-transition floor up ~5.5 KiB to
   26,112 B.
3. **Save path** (earlier today): streams four 8 KiB banks
   (`export_sram_bank_into`) instead of one 32 KiB allocation; saving in
   Oak's Lab used to fail exactly here.

## Accepted risks (documented, not changed)

- **Battle-transition snapshot** (23,040 B `FrameBuffer`, the largest
  allocation in the battle window): requires the transition frame to have
  ≥23 KiB free. It is preceded by `resources.clear_cache()` and has been
  observed to succeed in every run at the 26 KiB floor (~3 KiB slack). Do
  not add retained allocations to the battle-entry window without
  re-measuring.
- **Boot SRAM image read** (32,768 B one-shot): boot has the emptiest heap;
  safe today. If the boot retained set grows, stream this too.
- **Per-frame churn, all freed within the frame**: move-animation tileset
  clones (2 × 5,120 B/frame while move objects are active,
  `render/battle.rs:4815-4819`), the horizontal-raster buffer (≤10,752 B,
  `battle.rs:3218`), Hall-of-Fame per-frame scaled tiles (3,136 B,
  `hof_ceremony.rs:299`), title/intro/Pokédex asset clones. Churn fragments
  the free list over time; none of it moves the per-phase floor today.
- **Map-transition cluster** (~6-15 KiB: MapJson materialization,
  script_config parse, script registry rebuild): runs with ≥30 KiB free;
  fine, but adjacent to the lab peak.
- **Game Corner slots sheets**: one-time 4.6 KiB retained when slots are
  first opened (`render/slots.rs`); acceptable.

## Re-measuring

Build with `--features repro-rival` (implies `autopilot` + `repro-markers`)
and run in mGBA; the log emits `repro: heap low <bytes>B at frame <n>` for
every new low-watermark and prints the free block on every screen
transition (`mk: transition to <screen> free=<bytes>B`). A future CI check
could assert the low watermark stays above a floor.

## Static sweep — top allocation sites (any phase)

| bytes | site | when |
|---|---|---|
| 32,768 | `game.rs` SRAM read image | boot (freed) |
| 23,040 | `game.rs:7315` transition snapshot `FrameBuffer` | battle entry (retained through the transition) |
| 16,384 | `battle.rs` `TileSet::blank(256)` | non-GBA-only path today |
| 10,752 | `battle.rs:3218` raster `Vec<Rgba>` | battle, per frame |
| 10,240 | `battle.rs:4815-4819` move-anim clones | battle, per frame |
| 9,408 | `intro.rs:78` gengar tile decode | intro |
| 7,168 | `title.rs:49` logo clone | title |
| 4,608 | `render/slots.rs` slots sheets | first slots visit (retained) |
