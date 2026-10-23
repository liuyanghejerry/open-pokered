# Hosted PCM sample clock

The hosted output previously advanced `CPU_CLOCK_HZ / sample_rate` integer
cycles for every stereo sample. At the existing 4,194,304 Hz APU clock, one
second of 44,100 samples advanced only 4,189,500 cycles; 48,000 samples
advanced 4,176,000. The discarded fraction slowed oscillator/envelope time
by about 0.115% and 0.436% respectively.

`AudioOutput` now retains the fractional cycle remainder for the lifetime of
the output stream. Both the device callback and embedding `render_pcm` use
that same renderer with the existing manager/APU. The renderer owns no
sequencer or audio channel state. A new output starts a new conversion phase;
register writes and ordinary logical updates retain the current phase.
CPAL uses its configured rate, Web Audio uses the actual negotiated rate,
and embedding output uses its existing 44.1 kHz contract. A stream has one
positive, fixed sample rate and complete interleaved stereo pairs.

The APU is still driven by output sample demand in this hosted path. This
change does not establish source CPU/IRQ time ownership, Game/HoF loading,
original boot timing, or whole-game PCM equivalence. GBA physical PSG output
uses its existing separate backend and does not allocate this renderer.

## Verification

- Advance one second at both 44.1 and 48 kHz and compare the entire APU
  playback state with an independent `tick_n(4_194_304)` reference. The
  former per-sample integer-floor path fails that comparison.
- Compare every PCM sample and final APU state for one large pull versus
  irregular callback partitions, including an empty pull.
- Perform a real register change between callbacks and compare each following
  sample with independently calculated rational sample boundaries.
- Exercise repeated public `AudioOutput::render_pcm` calls and compare the
  live manager APU against accumulated exact cycle counts.

These are library/device-contract tests. They do not claim a newly recorded
original or native player run. Logs and hashes are recorded in the adjacent
validation JSON after local verification.
