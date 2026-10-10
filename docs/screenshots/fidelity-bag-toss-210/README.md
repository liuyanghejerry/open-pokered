# Bag TossItem confirmation and result: standalone 210

Actual masterb9f47fa removes items immediately after selecting a quantity, without
the original IsItOKToTossItemText, YES/NO or ThrewAwayItemText. Important items
also incorrectly enter quantity selection and then return to the overworld to
refuse. Four changed production files implement this one Bag TossItem flow:
core state machine, shared PokemonGame dispatch, renderer and retained cache.
No PC menu, Bicycle action shortcut or extra CANCEL entry changes are included.

Original primary TossItem_ calls PrintText for a question ending in PROMPT, so
this question must be acknowledged with A/B before DisplayTwoOptionMenu. This
is intentionally different from inner DONE auto-return. Default YES is index0;
B wins an A+B chord. Both outcomes wait15 frames, during which input is ignored.
Only YES then removes the chosen amount; the result is another blocking PROMPT.
NO/B return to the list unchanged. IsKeyItem/IsItemHM skip quantity and refuse
in the item menu. New text honors configured1/3/5 letter waits and held A/B
gradual acceleration, and acknowledgement/choice request PressAB through the
shared audio manager. No original PCM/waveform parity is claimed.

Fresh original207 recording starts from the controlled pre-PC field emulator
state and sets a single POTION stack4, cursor0 and Medium speed, then uses real
START/item/action/quantity inputs. Quantity is2; A397 starts question printing;
PROMPT waits472, acknowledged492; default YES remains0. YES at542 leaves stock4
through556 and RemoveItemFromInventory557 changes it to2. NO/B/A+B remain4.
Four cases repeat twice: all5544PNG, complete JSON and execution hooks byte exact.
The first a1 recording omitted the PROMPT acknowledgement and never reached the
choice; its log is archived and full raw diagnostic remains in the workspace.
It is evidence of the missing input, not a completed successful branch.

Actual masterb9 baseline app production prefix is verified unchanged and the
byte-identical test helper is appended. Baseline compatibility observations
return no Bag dialogue because none exists there; final inherent state methods
provide actual new observations. The normal owner regression runs1 test on the
baseline and FAILS at32: stock2 rather than4. Final full suites pass core2717,
app192 (19 recording tests ignored by default), including that same regression.
Initial a1 failed only a helper u16/u8 assertion mismatch; successful a2 rebuild
and full test logs supersede it, and both logs are retained.

Before records precede after records. Controlled actual PokemonGame starts in
Bag with English/Medium and POTION4 (or HM01 for refusal), with real per-frame
button edges: A0, DOWN8, A16, UP24, A32, question acknowledgementA120, optional
NO DOWN150, answerA/B/A+B170, receipt acknowledgementA270. YES stock stays4
through184 and commits2 at185, exactly15 frames after selection. Four branches
plus HM refusal each record301 frames and repeat twice on both sides. Every
6020PNG and complete recorded JSON (phase, actual inventory, full Bag/field
dialogue state, arrow, screen and hardware input) repeats byte exact, no masks;
both sides have identical inputs. Every frame compares all retained/full pixels.
The unit tests also inject ignored input during the protected wait, cover
quantity chord priority, and verify key items/HMs retain stock and return to Bag.

This is a seeded, direct-Bag owner scenario, not a fresh mainline/save/Continue
proof. Original and native entry CPU/PPU timing, fonts and layouts differ; no
absolute original/native frame or pixel alignment is claimed. Original quantity
tile retention/font restoration and wider menu/input timing audits remain open.
Cache keys include printed text and arrow state. Current-head CI and GBA budgets
must independently pass without threshold changes before merge. Archive includes
all raw records, source, primary assembly, scripts, full logs and source/frozen
binary hashes, with no ROM, emulator state, SRAM or executable distributed.
