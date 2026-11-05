# Indigo Plateau Party return follow-up

> 存储说明：逐帧录制、压缩日志和配套运行报告已移出版本库。下文的录制文件名及哈希仅描述历史验证；复现输出请放在 `.task-tmp/` 或仓库之外，完成后清理。见[审计材料约定](../../../README.md)。


The fresh route exposed two real failures at Indigo Plateau Lobby. The earlier
Party candidate stopped on `MissingSource(Sprite(41), 0)` when B cancelled the
menu. Fixing that source range revealed a second failure in the default GBA
ROM: allocating the expanded OAM instruction vector exhausted the heap and
left the display white. Both failures reproduce from the ordinary SRAM
fixture here; the fixture contains Zapdos, Lapras and Venusaur.

## Changes and source checks

- Original sprite metadata gives the lower-copy length, not the complete
  reload range. Twenty moving sprite IDs have twelve-tile PNGs. Their upper
  reads continue into the next resource in the same original ROM bank.
  Resolve those spans by bank/address, preserving alias IDs within a sheet.
  All 72 IDs (23,808 requested bytes) match the original ROM, and the native
  PNG decoder and embedded GBA assets agree. This includes the guard span
  across three adjacent four-tile resources; no padding or repeated frames.
- OAM instructions now use a three-byte tagged representation, versus twelve
  bytes previously. Count the same expansion before allocating once, avoiding
  a transient old/new vector pair. The 44 existing original/constructed
  contracts retain every instruction cycle, access, ordering, delayed effect,
  refusal cursor and snapshot continuation. The trace checksum fixture pins
  pre-change instruction sequences; the existing original-ROM fixtures remain
  the independent outcome/cycle oracle.

Production source is `cf65687f424dbe95a12c75d97615e4da5a18e1ae`, including
master `fc04f261` and the short-sheet fix `1e927ea2`. `verification.json`
records binary and fixture hashes. Original source/ROM identity is also in
`crates/pokered-app/src/field_sprite_patterns_1390.json`.

## Actual input verification

Native bug/candidate pairs have identical initial Party images and semantic
states. Cold boot through CONTINUE, START → POKEMON → B, then 160 consecutive
logical frames: neutral and early Down/A, each repeated twice. The old build
stays in Party with the source error; the candidate returns to START at tick
35, keeps the player and party unchanged, and discards the early pulse.
Release/debug builds agree on all frame states and captured images.
Compressing OAM storage leaves all 160 frame states and captured images
identical to the short-sheet-only fix, in both repeats. SRAM is unchanged.

Default GBA (no diagnostic features, autopilot, frame skipping or RAM writes):
ordinary SRAM boot and physical keys, 600 consecutive physical frames. Neutral,
short Down at ticks 100–101 plus A at 106–107, and held Down at 100–339, each
repeated twice. Every image hash repeats. Neutral/short sequences are identical
and stay on POKEMON; held Down selects ITEM after restoration. Closing START,
walking and reopening work. No allocation panic or restoration error remains.
The old/new GBA screenshot pair uses neutral input and the same tick 599.

**Timing limitation:** this crowded GBA map settles at tick 316, or 317 physical
frames / 5.307 seconds from the first B input. The earlier one-party Rock Tunnel
case's 227 frames are not a bound for all maps. Original-speed or full physical
clock parity is not claimed. Prepared controls do not count as a fresh route.

Local checks: 1,086 application test executions (92 existing ignored), 3,987
core tests (1 existing ignored), ARM release check and default ROM build pass.
A new unseeded m01–m49 run is in progress using the current release binary;
earlier failed or superseded runs are not reported as passes.

## Reproduce the native regression

From the repository root, build the debug-server native binary, then run:

```sh
python3 docs/audits/2026-11-05/party-return/indigo/replay-native.py \
  --binary /absolute/path/to/pokered-app --sha256 <binary-sha256> \
  --output /absolute/path/to/new-output-directory
```

The committed fixture is only setup; formal captures use normal CONTINUE and
physical menu inputs. `--expect-bug` asserts the old nurse source failure.
The replay enables PCM sequencing while muting host audio output. See the
compressed frame records and screenshots under
`docs/screenshots/party-return-1390/indigo-*.png`.
