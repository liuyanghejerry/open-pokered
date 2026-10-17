# Bike question preparation input and text delay

Holding A immediately after advancing the final Welcome with B used to consume
the press while the question was printing. The list then waited for another fresh
press. The original caller sets `NO_TEXT_DELAY` for this question and neither
PrintText's three preparation waits nor the cursor's three waits poll Joypad.
The pending A therefore remains fresh for the first list poll. Accepting A
clears the bit; returned B preserves it. A-on-CANCEL and B are distinct paths.

`before.png` and `after.png` both show relative frame +66 from the final Welcome
prompt: B held for two frames followed immediately by A held for 64. Before
still waits at row 0; after has accepted A and printed the unaffordable reply.
Money stays 1000, the bag stays empty, and flags stay unchanged.

The complete tapes differ before that milestone: the legacy five authored
lines produce three layout pages, whereas the original PARA continuation has
two paragraphs. Both use the same physical SAV, English, seed 42, and the first
670 literal boot/Continue/talk inputs. Each prefix acknowledges its visible
Welcome pages normally, with a 64-frame neutral reading pause. Their final
Welcome anchors are 1043 and 1030, and their final frames are 1109 and 1096.
The suffix is identical; the full vectors and absolute frame numbers are not.

The SAV is reused from
`docs/validation/field-question-ack-1075/fixtures/bike-sale.sav` (no new copy).
`validation.json` pins source revisions, input/image hashes and actual checks.
The normal CLI runs used no setters, warp, skip, snapshot restore or SAVE;
shutdown after the diagnostic boundary was owned TERM cleanup. CLI audio was
disabled; actual sequencer/PCM checks belong to the separate App tests.

Before was recorded on `3ada1c5272bce1a402d020380fec40bf092e6af5` and after on
`63faa0f91bd4011c2247ea2729106d84047ca7fc`. The branch subsequently merged master
`e28f59bfe255eb3d7ab1318c240596f95c736438` as
`b1439b38ff83b7eeddaeb08dbdafa1b3a8a59eec`. All added/removed Bike lines in the
same 20 source files are identical after that merge; one adjacent-insertion
conflict preserved both independent bedroom-script and Bike status-byte writes.
Earlier runtime results are not relabelled as execution of the merged source.

Local checks: Data protocol and Boa host each passed once on `53aad5a8`;
Bike core 7, Agent 58, and the App Bike test under default/debug-server each
passed on `63faa0f9`. Each App test contains six constructed cold cases, rather
than six standalone normal CLI boots. An earlier full-core run on `b0d687a2`
had 3078 passes and two test-fixture/driver failures; both failed names passed
in the later seven-test Bike run. There was no later local full-core rerun.
Full CI must validate the published integration head.

For reproduction, build the normal CLI with `--features debug-server` from the
exact source revision selected above, then run this helper from its published
location against that checkout and binary:

```bash
python3 replay.py --case after --source-root /path/to/source-at-63faa0f9 \
  --binary /path/to/built/pokered-app --output /path/to/new-output-directory
```

Use `--case before` with the M11 checkout/binary for the baseline. `replay.py`
is a standalone literal-input replay helper with no private archive dependency.
It was syntax-checked; the published images came from the original recorded
runs. It records the supplied binary hash and validates the source checkout,
but does not itself establish which compiler invocation produced that binary.
No original-ROM CPU or total-frame parity is claimed.
