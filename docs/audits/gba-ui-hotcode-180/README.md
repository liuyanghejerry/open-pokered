# GBA UI helper instruction placement

Move the existing opaque UI glyph painter to the existing IWRAM block. This split PR is based directly on master `31b1eda` and changes only one linker selection rule. Function implementations, fonts, frame output, input handling and gameplay are unchanged. No new screenshot pair is required for instruction placement alone.

The verification JSON and evidence archive below are retained historical measurements from the larger #144 branch. Their source/ELF hashes identify that context; they are **not** measurements of this new master-based split. In particular, the larger branch's gameplay gates do not apply to this isolated change. This split must pass its own exact-head GBA production build, unchanged performance budgets and other CI before merge.

## Historical evidence

# GBA opaque UI tile hotcode (180)

The Pokemon opaque UI tile painter still fetched instructions from cartridge ROM, while the older generic border helper was already in IWRAM. Add the current helper to the existing linker hot framebuffer block. Its implementation, glyphs, colors, layout, timing logic and framebuffer writes are unchanged. This is instruction placement only and changes no screen output; no additional visual comparison is required. The PR retains the earlier gameplay/rendering comparisons.

Final before/after GBA builds match 59 runtime-source hashes; only the linker rule changes. The original performance baseline and all 31 budgets are unchanged. Oak dialogue draw average falls from 3008 to 2985 ticks, below 3004.9. Movement draw peak remains 3122, above 3113.1: **30/31 pass, not merge-ready**. Two complete measurements of the final frozen ELF produce byte-exact result JSON. Samples and render counts in all seven scenarios match before.

The final perf ELF places the 396-byte helper at 0x03001b8d, with IWRAM end 0x03004e10 (19984 bytes). The original 24320-byte cap and 8 KiB startup/IRQ stack reserve remain. Both benchmark and normal production release builds pass the unchanged linker cap. Sources, ELF hashes, raw timing/compare logs and symbols are in verification.json and evidence.zip. Native runtime sources remain unchanged from the full debug tests in batch179.

Negative trials are retained: outlining PC keys was ineffective; ink-run batching slowed Oak; clipped tile placement did not change movement; outlining the full resolver slowed movement; outlining only border lookup exceeded the unchanged memory cap; separating animated rows fit but slowed movement. **All those source/refactor rules are withdrawn.** Final production code adopts only the demonstrated UI helper placement. No performance threshold was relaxed.

Remaining: localize the movement peak before further optimization, fix the independently reproduced PC quantity prompt handoff (batch181), finish other documented fidelity gates, final-source mainline and independent SAVE/CONTINUE, and exact latest-head green CI before merge. The frozen batch179 new-game run has passed m01..m11 and continues; it is not a final-head completion claim.
