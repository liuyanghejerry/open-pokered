# Battle nickname fidelity

Giving Bulbasaur the nickname `SPROUT` previously left its battle HUD,
send-out and move text displaying `BULBASAUR`. Battle-name formatting now
reads the original party identity and decodes a stored nickname. Names stay
separate from the mechanics engine's species: Transform keeps the original
name while copying battle stats and appearance. Names without nicknames
continue to use canonical species spellings.

The shared name context is used by production turn narration, including
status, residual damage, fainting, charge/recharge and Substitute text. The
same party identity supplies the HUD, send-out/recall, already-out refusal,
trainer switching and link opponent switching/escape messages. Ghost and
Old Man special HUD labels keep their existing branches.

## Original reference

Reference revision: `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.

- `home/text.asm:144–174`: move user/target use `wBattleMonNick` and
  `wEnemyMonNick`; opponent names receive the Enemy prefix.
- `engine/battle/core.asm:1692–1700`: enemy party nickname loads into the
  active battle name; `1815–1827`: player HUD uses `wBattleMonNick`.
- `engine/battle/move_effects/transform.asm`: changes battle identity,
  stats and moves without replacing the nickname variables.

These are source comparisons; no matched original-ROM rendered nickname
recording is claimed.

## Verification

A legal Bulbasaur12 save with Tackle/Growl and nickname `SPROUT` was imported,
then played through a real wild Rattata10 battle using button input. Saving
and importing the pre-battle SRAM confirms the encoded nickname persists.
The base binary's source tree matches master `99dd575d`; the fixed binary
SHA256 is `503ab0f626ef1839862b7ed8059ed51154530b1c21bb4b0af23905f3e9979fcd`.

Two before and two after replays capture the same frame544 and message page.
Each version's two images are identical. State rows and message sequences
match after replacing the old species name with the nickname. All340 changed
pixels are inside the HUD-name and narration-name regions; sprites, HP and
other rendered areas are identical. See `replay-comparison.json` and the
committed screenshots.

Five core regressions cover real turn/recall/enemy-free-move paths, named
status/faint messages, original identity during Transform with and without
a nickname, canonical unnamed species spelling, and real poison/recharge
messages. All2,845 core tests pass. The debug application builds successfully.
A fresh power-on playthrough through m01–m06 also passes on the frozen fixed
binary, reaching OaksLab(5,6), overworld, no active script effect at frame10955.
This is scoped main-flow regression, not a new complete first-clear claim.
