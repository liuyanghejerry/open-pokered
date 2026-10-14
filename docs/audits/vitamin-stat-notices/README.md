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

Latest native comparison is master1b5f69ae versus head d0e94f05, after
integrating the vitamin numeric fix PR230 on both sides. The same five initial
fixtures use actual START/ITEM/USE/party, Stats, bag and ordinary SAVE/YES,
two runs per case/build (20 total). All notices capture frame640 with identical
actions. Repeats have byte-identical PNGs. All four corresponding SRAM files
for each case have the same SHA256; independent imports retain party/PP,
cash3000 and quantity1. The only production change is the notice text.

All five native cases now preserve currentHP10 and match all43 modeled
original party bytes, including updated StatExp/cached stats. Byte7 (catch
rate) is excluded: original45 versus native0 is a separately confirmed save
issue. These checks do not claim original whole-save or animation parity.
The older master3568 captures (HP UP currentHP11) remain private historical
evidence and are replaced by the current-master20-run set in this PR.

All23 existing bag-use tests pass on the updated branch; the debug-server
application builds for both the exact master and updated branch. The compact
verification.json records image/binary/fixture/SRAM hashes and outcomes.
Private original/native evidence roots are located through
.task-tmp/fidelity-39-plus/audit-849-original-notices-path.txt and
audit-849-current-master-native-path.txt.
Scripts and logs are audit-849-original-vitamin-notices.py,
audit-849-current-master-native-capture.py, their success logs, and
audit-849-current-master-bag-tests.log / audit-849-current-master-app-build.log.

Initial source runs incorrectly tried to hook a text-data label as executable
code; those failed runs are retained and excluded. The first native harness
incorrectly expected the unfixed HP UP to preserve currentHP; that failed
run is also retained separately and excluded from the20 successful runs.
