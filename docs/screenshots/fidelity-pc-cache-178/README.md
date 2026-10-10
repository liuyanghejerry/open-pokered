# Retained PC message frames

The PC renderer acquired typewriter/CONT/PARA behavior earlier in this PR, but `PcVisualKey` still hashed a fixed four-line source slice without the visible character count. The normal retained renderer therefore reused its empty first frame while the game advanced the text. Direct screenshot/full-draw probes did not exercise this cache.

A real `PokemonGame::update` + `RenderSession::render` regression first failed at frame 2, character 1, pixel (8,115). At frame 10 the core had three characters but the retained screen was blank. The repaired key hashes visible characters, actual authored page contents, and the warning's selecting-menu state. It retains reuse on visually unchanged frames.

The owning regression follows opening → Bill's PC → CHANGE BOX, including the original warning's CONT/PARA inputs, and compares every retained pixel with a full redraw after every game update. Both complete repaired runs pass. Opening captures and states repeat byte-exactly. This proves cache consistency, not full ROM timing or intermediate scroll fidelity.

Screenshots use the same English RED PC constructor scene and eleven idle owner updates (frames 0–10). `master-before.png` is actual master31: its earlier PC implementation shows the whole message immediately. `cached-bug-before.png` is parent aa66399's retained blank-frame bug. `cached-after.png` is the repaired retained frame, identical to the repaired full draw. Master captures have two identical copies; its production session prefix is byte-identical to master, with only a test capture helper added.

Native final a1: core 2750, data 263, app 223 (33 ignored), agent 54, audio 99 pass. Source, before failure, screenshot/state pairs, tests, manifests, exact-head parent CI failure and GBA evidence are retained in `evidence.zip`; hashes are in `verification.json`.

GBA still fails two original budget metrics: oak draw average 3008 against limit3004.9, movement draw peak3122 against limit3113.1. All31 metrics and 59 exact source hashes are retained; no budget change. This repair is not a performance fix, merge approval, full mainline proof, or completion of the broader fidelity goal. Previously recorded draft gates remain open.
