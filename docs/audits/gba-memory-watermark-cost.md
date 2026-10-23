# GBA diagnostic watermark cost

Two independent CI runs exhausted their limits while making progress. The
hosted-audio-only PR364 spent 32 minutes downloading 109 MB of emulator
installation dependencies and hit the unchanged 35-minute job limit. PR363's
retry reached move157 of165 (both sides) before the unchanged600-second broad
memory-suite timeout. Its last observed contiguous low was4640B; the remaining
moves and scenarios were not completed, so this is not a passing memory result.

The diagnostic previously bisected the full256KiB request range on every game
frame, making about18 fallible allocations even when no new low was possible.
The log only publishes the running minimum. The new probe first attempts a
real allocation of that previous minimum on every frame. If it succeeds, the
frame cannot lower the watermark. If it fails, the same byte-sized binary
search finds a new low below it. An unchanged positive low therefore costs one
allocation, not a full search. A one-frame drop is still tested and retained.

The returned value is a running low, not the instantaneous largest free block.
Rare checkpoint reports still use the existing full probe. Probing itself can
perturb allocator layout; the new allocation history is intentionally less
intrusive, and new CI measurements must establish the resulting margin. No
previous heap values are copied or substituted into a passing result.

The installer requests only required dependencies for the same pinned/checksummed
mGBA build, plus explicit`xauth` for`xvfb-run`. It omits recommended packages.
All original workflow stages, memory scenarios, both-side165-move sweep,
heap/stack gates, hardware timer thresholds, rendering checks,35-minute job
limit and600-second suite limit remain unchanged. The normal game build does
not include this diagnostic probe.

## Local checks

- Portable Rust tests exhaust all previous lows0..512 against capacities0..1024,
  exercise exact4095/4096/4097 boundaries and the initial ceiling, and prove
  every stable frame still calls the allocation predicate.
- The existing27 performance/memory/frame-timing tool tests pass.
- The GBA`memory-scenarios` target passes locked release checking.

The full original CI remains the acceptance gate. These local checks do not
claim the complete memory/scenario run has passed. Commands and log hashes
are recorded in the adjacent validation JSON.
