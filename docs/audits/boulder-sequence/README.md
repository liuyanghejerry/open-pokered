# Boulder slide, dust, and control lockout

Holding toward a Strength boulder currently lets the player begin walking while the original is still sliding the stone and showing smoke. This patch retains the stone at its origin for presentation, commits its logical destination before visible movement, slides one pixel per field sample, and locks player controls through smoke and graphics restoration. It separates retained LCD output from logical control resumption and snapshots in-flight push state.

Dust follows original sprites 36–39, including raw coordinate clipping, OBP1 palette, OAM priority, and the original vertical clipping attribute bug. Victory Road 1F/2F switch flag/block replacement runs during the slide; visible block replacement adds its redraw pause. Seafoam holes and Victory Road 3F landing effects remain at completion.

## Verification

Four ordinary directions: two deterministic native captures each; all 201 frames match the corresponding original recording without resampling. Victory Road 1F and the first 2F switch also match all 201 frames. These are controlled save/map fixtures, not full-playthrough evidence. Repel/native setup and encounter-suppressed original setup differ; no RNG or audio parity is claimed.

Core: 40 executable suites, 3,248 passed tests. App: 41 executable test programs passed. Snapshot tests compare interrupted/resumed pushes with uninterrupted execution at five phases and accept legacy snapshots missing the new fields. The debug control_ready regression waits through the lockout. Local GBA release memory-scenarios build passed; CI runtime, coverage, and performance checks remain required.

## Known remaining fidelity differences

Victory Road 2F second switch has 27 differing frames. Victory Road 3F hole has 33 differing frames; its event occurs at input frame 73 here versus 72 in the original. These findings remain open and are not reported as fixed by this PR. Source-only CPU experiments show that IsObjectHidden scanning can cross VBlank and delay the first step on the same map; they do not prove native parity. The corrected workload report includes the interrupt-entry, vector jump, and RETI cycles omitted by routine-boundary hooks.

Before/after images are captured from master 02b9d9a2 and this branch with the same save fixture, Repel/Strength setup, and hardware frame offset after directional input. See screenshot metadata for executable and frame identities.
