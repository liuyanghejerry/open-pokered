# PC text, warning and confirmation restoration

> 归档说明（2026-11-11）：原始录制包、逐帧日志和运行报告已从当前源码树移除。下文保留历史结论与验证范围；旧附件可在清理前的 Git 历史中查阅。复测请使用保留的脚本，将生成文件写到临时目录。

Master `303aaceb` printed ordinary PC messages instantly. CHANGE BOX and
RELEASE entered YES/NO immediately; an early A could already enter the box
chooser or commit a transaction before the warning was read.

This independent split of #144 restores ordinary PrintText letter waits,
fresh prompt input, authored paragraph/CONT transitions and DONE returns.
CHANGE BOX, RELEASE and TOSS keep their prior menu visible underneath and
reach the existing YES-default confirmation only after their warning.
Opening/subsystem/shutdown and item-transfer messages wait for their real
sound owner where the original caller does. Key items follow that same path
when they bypass quantity selection. The retained renderer keys the visible
text and warning underlay. Existing fonts, localization and UI layout remain
outside the fidelity comparison.

Original ROM control (audio emulated, read-only routine hooks, no RAM/ROM
writes during recording): CHANGE BOX waits for text input at relative indices
6 and 20 and reaches YES/NO at 47 after A at 7/8 and 27/28. An early A at 4/5
never reaches YES/NO in 93 observed frames. Both cases repeat exactly.
The native candidate repeats those outcomes at the same relative indices,
with unchanged SRAM until confirmation. Master enters YES/NO at index 0;
the normal text-ACK sequence already writes SRAM, and the early sequence
reaches the box chooser. These are paired semantic cues, not a claim of equal
absolute physical clocks or font metrics across emulation and native code.

Native evidence uses ordinary SRAM and cold CONTINUE. Master/candidate
initial screenshots match. CHANGE BOX, RELEASE and TOSS each complete twice;
six independent CONTINUE processes re-save their actual runtime stock and
match the expected box/item/party data. Source-only test fixtures now release
A between acknowledgements; existing text, inventory and cursor assertions
remain, with bounded loops that fail rather than hang.

Validation: app 1,014 test executions (92 existing ignored, including duplicate
library/binary suites), core 3,811 (1 ignored), TUI 6 (6 ignored), ARM release
check and native build pass. A fresh NEW GAME through m06 passes with seed 42;
this guards the shared dispatch and is not reported as a complete first clear.
Non-linear PC coverage is the separately recorded SRAM suite above.

The compiled native binary is
`3bda91fabaec9afa22073d1c6f0b4bf5feb7130430ffd56da0b0824d20b80cbe`;
all 2,380 crate/Cargo files recorded for the build match code commit
`cf1c2d461aa8c258dabc1f195708c11da5862a04`. Before/after screenshots under
`docs/screenshots/pc-messages-1390/` use identical inputs and relative capture
indices. Compact verification is in `verification.json`.

The default release GBA ROM also passes two ordinary cold SRAM runs. The
4,718 physical frames exercise PC access paragraphs, an early warning pulse,
two warning acknowledgements, cancellation, log off and walking. All 448
recorded warning-frame hashes and every stage image repeat exactly; SRAM
remains unchanged. This is an input/state control, not original-speed parity.

`replay-native.py` and `replay-transactions.py` reproduce the prepared-save
controls with an explicitly supplied `--binary`, `--sha256` and `--output`.
Use `--candidate` for the warning driver to assert the restored behavior.
The included 32 KiB `fixture.sav` is the same generated snapshot/SRAM fixture
used on master and candidate; it does not count as fresh story progression.
