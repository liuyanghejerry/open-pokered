# TM prize-counter dialogue

Base: `d037ea54abdf5d76e950e1c08fb560cebd7fa441`.

The TM counter printed an extra “TM prizes…” price explanation between the
exchange greeting and the question. Remove that dialogue so all three counters
follow the original sequence. Prices remain in the choice entries.

Original source: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`engine/events/prize_menu.asm` `CeladonPrizeMenu`: exchange greeting → prize
menu preparation → WhichPrizeText → HandleMenuInput. Text is in
`data/text/text_2.asm` (`_ExchangeCoinsForPrizesText`, `_WhichPrizeText`). Neither
prints a separate TM-price explanation. The original TM costs in
`data/events/prizes.asm` are 3300 / 5500 / 7700.

The new script regression fails on the unchanged scene and passes after removal.
All 81 native-script tests pass, including the existing delivery-before-charge,
confirmation, list exit, and full-inventory checks. Both game versions and all
three counters are covered by the new dialogue-order test.

Actual CLI checks use the supplied offline persistent fixture, normal boot and
CONTINUE, then buttons only. No warp, state setters, or mid-run snapshot restore.
Two base runs and two candidate runs reproduce identical observations within each
side. Candidate checks: entry, list cancellation, confirmation NO, insufficient
coins, full bag, each of the three successful TM purchases, and missing COIN CASE.
Purchases verify actual item/quantity and exact coin deduction; refusals preserve
bag and coins. Both screenshots use the same input sequence and frame 926.
The regular s07 independent-process SRAM roundtrip also passes; it is a general
save check, not a claim that the prize fixture traveled from a fresh game.

Reproduce from the workspace root (debug-server binary):

```sh
python3 docs/validation/prize-tm-dialogue/replay.py after /path/to/pokered-app /path/to/new-output
```

`before-*.json.gz` and `after-*.json.gz` retain complete command and observation
records. `verification.json` records executable hashes and paired-frame identity.
`unit-before.log.gz` intentionally contains the original failing regression.
Original ROM input, audio, animation, and whole-game fidelity are outside this
small change's verification scope.
