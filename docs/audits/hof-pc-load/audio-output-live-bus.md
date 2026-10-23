# Live APU access in channel helpers and note pitch

`ChannelOutputInstructions::try_step_with_bus` and
`NotePitchInstructions::try_step_with_bus` execute the existing CPU cursor with
an `ApuInstruction` callback. Both take explicit `NoteBusInput` command-result
values and return `Result<bool, BusStepError<E>>`. The old CPU-only methods and
store-only helper API remain available and their fixtures are unchanged.

The callback owns the only clock and APU. It either rejects before any side
effect, or accepts the entire instruction and returns the actual read value
when requested. A successful read without a value violates this owner contract;
it is not converted into a retryable error after clocks have already advanced.

The live helper emits both original NR51 reads: `EnablePrefix(3)` and, on the
stereo-panning path, `Panning(5)`. The latest successful read is retained in the
same CPU cursor until `EnableStore(1)`. Music without a paired SFX and SFX noise
apply panning to the second value; other SFX and covered music set their channel
bits in the first value. NR51 is no longer supplied as a pre-read scalar to this
API. The old store-only API retains its prior scalar-input contract.

The pitch continuation propagates access instructions through its existing
helper children, and emits its own envelope and rest stores. A wave rest reads
NR51 in `RestWave(3)` and later writes the retained value with bits `$44` cleared
in `RestWave(5)`. Pulse rests write envelope `$08` and restart `$80` at their
separate original instructions. Audible notes propagate volume, duty/length,
enable, wave upload, frequency and cry-modifier stores.

Both cursors expose `requires_wave()`. `MissingWave` is returned before any
instruction when a required wave pattern is absent, including after resumption.
Pitch requires it only for a non-rest channel-3 note without a covering SFX;
rests, covered notes and pulse channels accept `None`. Successful reads and
partial child state serialize with the existing cursor, so restoring between
read and write does not sample NR51 again.

## Independent source validation

`generate-output-live-bus.py` runs the bounded original-ROM oracle, recording its
actual accesses. `source_apu_access_oracle.py` supplies changing hardware NR51
samples at the original read's invocation; the shared oracle records the reads
and writes. The wrapper contains no opcode/control-flow interpreter. Access
offsets come from an explicit original-opcode table: LDH's third machine cycle,
LD `[HL]`/`[HLI]`'s second, and absolute LD's fourth. These are checked against
each original instruction's T-cycle duration.

The 1,121,494-byte fixture references existing instruction traces by case index
rather than duplicating them. It covers 930 helper calls and 2,100 complete pitch
continuations with 25,218 APU accesses in source banks 2 and 31. All helper paths,
envelope/rest/duty/wave outputs, paired-channel suppression, cry modifier edges
and perfect-pitch behavior retain the original CPU fixtures. Three scalar input
sets include different first and second NR51 samples, so reusing the first
panning read is detectable.

Tests compare every instruction, address, value, offset and access order. Every
instruction is refused twice, then serialized/resumed and accepted. Additional
checks reject missing wave data before initial or resumed execution. Regeneration
after enabling the shared oracle's optional `read_events` produced identical
fixture bytes, SHA-256
`71bf7af97f500bee0ae4a72ec7326324d0d23ce2611129467b4c3562b3dee48d`.

Source ROM SHA-256:
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`;
source revision: `pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
Only controlled audio routines are executed during fixture generation. No
graphics, original font rendering or gameplay replay is involved.

This change propagates real bus access requests. It does not itself attach the
source clock to Game, replace device audio ownership, or prove a player-visible
Hall of Fame loading fix.

`output-live-bus-validation.json` records the exact component test filters and
GBA check with log paths and SHA-256 hashes. Logs live under the existing
`.task-tmp/fidelity-39-plus/audit-1275-*` directory entries and are not committed.
Whole-frame manager validation is listed separately in
`audio-live-bus-integration.md`; the broad preliminary `live_` test filter is not
counted as an additional independent suite.
