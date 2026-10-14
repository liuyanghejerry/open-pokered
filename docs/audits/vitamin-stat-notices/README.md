# Specific stat names in vitamin notices

The field bag's vitamin result now names the raised stat rather than saying
"stats rose!". The original VitaminStats table names HP UP's stat HEALTH,
then ATTACK, DEFENSE, SPEED and SPECIAL. The message now reads
"<Pokemon>'s\n<STAT> rose."; the notice layout and acknowledgement flow are unchanged.

Original reference: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c,
engine/items/item_effects.asm .useVitamin/.statNameLoop and
VitaminStatRoseText; data/battle/stat_names.asm and data/text/text_6.asm.
Retail ROM SHA1 ea9bcae617fdf159b045185467ae58b2e4a48b9a, PyBoy2.8.1.

All five original cases use ordinary START/ITEM/USE/party input from an
explicitly constructed initial state: two Lv12 Bulbasaur, DV9a78, lead HP10,
healthy partner35, zero StatExp and two of the vitamin. No RAM writes occur
after the input boundary. Each repeats twice: stable notice frame814 and
images/actions identical. A PrintText entry hook confirms the actual message
pointer; wStringBuffer contains the expected stat name. The healthy partner
and currentHP stay unchanged, the selected StatExp becomes2560, and an
acknowledgement consumes exactly one item. No original SAVE-menu claim.

Native comparison is master3568 versus source01ca4612. The same five initial
fixtures use actual START/ITEM/USE/party, Stats, bag and ordinary SAVE/YES,
two runs per case/build (20 total). All notices capture frame640 with identical
actions. Repeats have byte-identical PNGs. All four corresponding SRAM files
for each case have the same SHA256; independent imports retain party/PP,
cash3000 and quantity1. The only production change is the notice text.

The baseline's HP UP still changes currentHP10 to11; this numeric issue is
handled separately by PR230. This message-only comparison preserves that
behavior on both sides and does not claim original full-gameplay parity.
The original catch-rate byte is also outside this text change.

Existing bag-use tests pass; the debug-server application builds. The compact
verification.json records image/binary/fixture/SRAM hashes and outcomes.
Private original/native evidence roots are located through
.task-tmp/fidelity-39-plus/audit-849-{original,native}-notices-path.txt.
Scripts and logs are audit-849-original-vitamin-notices.py,
audit-849-native-vitamin-notices.py, their retry logs, and
 audit-849-bag-use-tests.log / audit-849-stat-notice-app-build.log.

Initial source runs incorrectly tried to hook a text-data label as executable
code; those failed runs are retained and excluded. The first native harness
incorrectly expected the unfixed HP UP to preserve currentHP; that failed
run is also retained separately and excluded from the20 successful runs.
