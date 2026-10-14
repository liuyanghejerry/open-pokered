# Preserve each Pokémon's catch-rate byte

An actual original ordinary SAVE loaded in the native game, followed by ordinary
START/SAVE/YES, changed byte 7 of both 44-byte party records from 45 to 0. The
remaining bytes of those two records matched. `deserialize_box_mon` discarded
MON_CATCH_RATE and `serialize_box_mon` emitted zero.

`Pokemon.catch_rate` now carries this per-instance byte through party and box
SRAM, JSON, PC movement, stat updates and trade. It is an optional byte so old
JSON lacking the field can derive the species' base rate while an explicit
zero or a non-species value stays authoritative. Newly created native Pokémon
store the source species' base catch rate. Complete Rust construction sites
initialize that field; struct updates inherit it.

Original source reference `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`:

- `engine/pokemon/add_mon.asm`, `.copyMonTypesAndMoves`: copies types and the
  next catch-rate byte from the mon header into party data. The byte is also
  the Gen-2 held-item byte in compatible records.
- `engine/battle/core.asm`, `LoadBattleMonFromParty`: copies the party prefix,
  including its catch-rate byte, into the battle mon.
- `engine/battle/move_effects/transform.asm`: Transform copies types, catch rate
  and moves from its target. Production battle resources and ball capture now
  use the mon's stored value, with the existing Transform override retained.
- `engine/pokemon/evos_moves.asm`, confirmed evolution: updates party species,
  stats and the two type bytes; catch rate survives. Explicit values stay
  intact. Legacy absence is resolved against the old species before an
  evolution changes identity, including the generic monster adapter.
- `engine/pokemon/add_mon.asm`, `AddEnemyMonToPlayerParty`: trading copies the
  mon record. Native trade preserves the field through the normal exchange
  and confirmed trade evolution.

Validation:

- Six focused tests pass: 33/44-byte records and JSON with 0/45/173; absent old
  JSON and new native creation; PC/stat/engine/evolution preservation; complete
  SRAM export/import/export; actual trade plus trade evolution; actual stack
  engine Transform with explicit 0/non-base values and preserved party identity.
- All 2,857 core tests pass; the native debug app builds.
- Two actual ordinary SAVE replays of the original 45/45 save and two replays
  of a 0/173 input constructed/checksummed **before boot** preserve every byte
  of both 44-byte party records. Readback JSON also retains the values. All
  four finish at frame 1268, and repeated saved SRAM hashes match per input.
  Compact evidence is in `ordinary-save-verification.json`.
- Fixed binary SHA256:
  `42260f63ecc04f141ffb634c7f1e37c912f909d1f35e96ab50e9a44517a11454`.

Hosted layout comparison using the pre-change field declaration with the same
unchanged field types measures 72 bytes, while current `Pokemon` is 76 bytes.
The current hosted `SaveData` is 26,896 bytes. The optional field increases
inline party/box storage; this report does not claim a local GBA build or budget
PASS. GBA checks remain part of the PR gate.

The updated PR includes master `72f9d593` (Rare Candy's fainted-use fix).
All six catch-byte tests and 2,858 core tests pass after that integration.
The recorded ordinary SAVE captures predate this unrelated item guard change.

A byte already overwritten by an older native save cannot be reconstructed
from a canonical SRAM record alone. Such a zero is indistinguishable from a
deliberately stored original zero, and evolution can preserve a rate different
from the current species' base value. This fix preserves explicit SRAM bytes;
the species fallback applies only to old JSON that has no catch-rate field.

This changes save data and game logic; no renderer/layout surface changes or
visual screenshot claim is included. The private source replay and logs are
`audit-851-catch-byte-roundtrip.py`, `audit-851-fixed-catch-roundtrip.py`,
`audit-851-catch-byte-tests.log`, `audit-851-core.log` and `audit-851-app-build.log`.

发布头 `fb184098` 的 CI 在 `agent_determinism` 集成测试发生默认工作线程栈溢出：普通客户端测试在 `script_engine_snapshot_round_trips_mid_cutscene` 中止，coverage 在 `fork_diverges_with_different_inputs` 中止。核心测试和 GBA ROM 构建通过。增加实例字段后，含整份游戏及存档临时对象的未优化测试超过默认 2 MiB 线程栈；这组测试现与其他全游戏测试一样使用显式 16 MiB 工作线程。全部输入、断言及测试数量保留，线程 panic 会传回测试框架，没有设置全局环境变量或改变生产代码。合并 master `6a0ef88b` 后，8 个测试分别在本地优化和未优化构建全部通过。最新 GitHub coverage 和完整工作流仍须另行通过，才可合入。
