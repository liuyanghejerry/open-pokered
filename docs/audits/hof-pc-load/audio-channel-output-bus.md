# Channel-helper APU stores

HoF PC picture loading remains **confirmed unfixed / unwired**. These retained
helper continuations now expose their actual APU stores alongside the owning CPU
instruction. They do not yet schedule a complete audio frame or drive Game/APU.
Instruction association is verified; subinstruction hardware bus timing is not.

`ChannelOutputBusInstructions` wraps the existing channel-output CPU cursor.
It retains immutable owner-supplied delay/duty, NR51/panning and wave bytes. Its
callback accepts one instruction and its optional store together; refusal must
occur before either clock advancement or an external write. The cursor advances
only after success. Serialization retains the exact instruction boundary, and a
completed helper calls nothing again. It neither copies/ticks a sequencer nor
owns an APU or another clock.

The stores cover duty/sound length, channel routing, disabling wave output,
copying all sixteen wave RAM bytes, re-enabling wave output, low/high frequency
and the cry modifier's second frequency pair. Channel 3 skips duty; pulse SFX
skip stereo panning; channel 8 still applies panning. Required unknown inputs
reject rather than receiving default zero values. The owner must supply actual
port/source values and keep them exclusive until the helper returns.

A separate pinned-ROM fixture generator records actual writes from the original
interpreter, including instruction index, PC, address and value. It checks 930
calls across Audio1/3, all eight channels, three port/duty/wave contexts and cry
carry boundaries: 4,320 APU stores. Original hardware address/mask tables and
wave bytes are read from the pinned ROM. Every non-ROM read is explicit. The
existing 310 instruction traces and return values are also matched without
removing their comparisons. Tests refuse and serialize every instruction and
compare the complete ordered store sequence, including repeated writes to the
same address. No graphics or fonts are decoded or rendered.

```sh
python3 docs/audits/hof-pc-load/generate-channel-output-bus.py \
  /path/to/reference.gbc /path/to/pokered.sym \
  crates/pokered-audio/src/source_clock/channel_output_bus_golden.json
cargo test -p pokered-audio --offline channel_output_instructions::bus
```

The next integration must propagate these stores through note/confirmation and
whole-frame continuations, add the other store sites, and provide actual source
ports and write timing. The native sequencer must then consume one logical frame
without replaying its batch of APU writes. Its selected-channel trigger cleanup
and pending master-volume consumption must remain identical to ordinary updates;
APU snapshots or disabling the register sink cannot provide that guarantee.
Manager fade/volume and post-frame hooks require their own correctly ordered
handling. Whole IRQ, Game hookup and ordinary-input end-to-end proof remain.
