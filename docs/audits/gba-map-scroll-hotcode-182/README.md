# GBA map scroll instruction placement

Move the existing DMA cache scroll helper to the existing IWRAM block. This split PR is based directly on master `31b1eda` and changes only one linker selection rule. Function implementations, fonts, frame output, input handling and gameplay are unchanged. No new screenshot pair is required for instruction placement alone.

The verification JSON and evidence archive below are retained historical measurements from the larger #144 branch. Their source/ELF hashes identify that context; they are **not** measurements of this new master-based split. In particular, the larger branch's gameplay gates do not apply to this isolated change. This split must pass its own exact-head GBA production build, unchanged performance budgets and other CI before merge.

## Historical evidence

# GBA map scrolling instruction placement (182)

The remaining original performance budget failure was movement draw peak3122
versus3113.1. Perf-only instrumentation identifies frame4217, ordinary walking
in RedsHouse2F with camera(3,7) subx14. A second diagnostic divides its draw into
setup176, cache preparation1399, background661, foreground585 and outside303
ticks. These instrumented readings are diagnostic, not final-head measurements.

Place the existing GBA `dma3_scroll_indices` helper in the linker’s existing
IWRAM hot-code block. This helper scrolls the indexed cache and clears its
exposed strip. Its implementation and all shared renderer code remain unchanged.
The original 24320-byte cap and 8KiB startup/IRQ reserve are unchanged.

Both normal production and perf-benchmark release builds pass. In both, the
952-byte helper is at0x03002869; IWRAM ends0x030051f0 (20976bytes). The exact final
benchmark rebuild is byte-exact to the frozen ELF measured twice. The two full
performance JSONs are byte-exact, all31 unchanged budgets pass, and all7 scene
sample/render counts remain unchanged. Movement peak3122→3078; Oak average2984
also stays within3004.9. Baseline SHA is pinned in verification.json.

Earlier foreground restore/save placement (3117) and extra cache preparation
placement (3118) still failed and were withdrawn. All diagnostic source edits
were restored byte-exactly. The archive retains their evidence and the failed
GameScreen cast build. Final60 WT/perf source hashes match. ROM/ELF binaries are
not bundled; hashes and symbol tables pin the immutable local builds.

This is instruction placement only and changes no on-screen output, so no new
visual PR comparison is required. Latest-head CI, final native mainline and
independent CONTINUE, and the remaining fidelity audit gates are still open.
