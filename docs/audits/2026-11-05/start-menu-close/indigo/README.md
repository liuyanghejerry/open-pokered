# Indoor START-close regression

> 存储说明：逐帧录制、压缩日志和配套运行报告已移出版本库。下文的录制文件名及哈希仅描述历史验证；复现输出请放在 `.task-tmp/` 或仓库之外，完成后清理。见[审计材料约定](../../../README.md)。


The synchronized START candidate could draw the map while remaining stuck in
restoration. In Indigo Plateau Lobby, its VBlank actor input failed with
`MissingPatternSlot(1)`. Indoor loading constructs assignments one object at a
time; the caller incorrectly passed this partial list as the binding table
for every still-visible actor. Keep the entry image bindings until the new
assignment table is complete. The underlying input validator still rejects
missing bindings; no actor or sprite update is suppressed to hide the error.

The regression test includes a crowded Indigo position and requires reopening
START after returning, in addition to stationary actors, early-input rejection,
preserved darkness and an LCD that stays enabled.

Production source: `f482e58735e58277bc9a5d3e09a2b9f3e0ea1225`, based on master
`6a6fcfae` and the updated Party branch. Current local checks pass: 1,094 app test
executions (92 existing ignored), 3,988 core tests (1 existing ignored), ARM
release check and the default GBA build.

## Ordinary SRAM controls

The existing [Indigo fixture](../../party-return/indigo/fixture.sav) is shared;
no additional save copy is needed. It boots through normal CONTINUE at (2,5),
then actual START/B inputs start the verdict window. Old and new runs have
identical initial states and screenshots. Neutral, early Right and held Right
are each repeated twice over 160 consecutive logical frames, followed by Down
and START to prove continued control.

The old candidate consistently stops on the missing-slot error and ignores
subsequent movement/menu inputs. The fixed candidate returns to the map at tick
21, rejects the early direction pulse, accepts held Right after restoration,
walks and reopens START. Release and debug builds have identical recorded frames
and screenshots. The existing Viridian City neutral/short/held controls, twice
each, remain byte-for-byte identical to the previous accepted candidate.

Default GBA ROM: Rock Tunnel and Indigo, three input schedules twice each,
600 consecutive physical frames. All repeats match, neutral and short-input
image sequences are equal, and logs contain no panic or restoration failure.
Additional neutral controls twice on both maps reopen START *before* walking,
close it again and then move. This separates menu control from Rock Tunnel's
legitimate wild encounter after walking (an Onix battle in the retained run).
No autopilot, frame skipping or recording-time memory writes were used.

The new unseeded m01–m49 route is running against the frozen release executable;
this prepared-save evidence does not claim a completed fresh route. The existing
original/native timing qualification is retained: in the Viridian held-input
case the first committed step is original tick 46 versus native tick 48, even
though both release map control at tick 29. This is not a claim of complete
animation or GBA physical timing parity.

Reproduce from the repo root with `replay-native.py --binary <absolute-path>
--sha256 <binary-sha256> --output <new-directory> --candidate`; use
`--expect-slot-bug` for the old failure. `verification.json` contains identities,
test counts and results. Compressed per-frame records and the matched
`indigo-reopen-before/after.png` screenshots retain the review evidence.
