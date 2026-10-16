# Silph 11F 火箭队离场验证

修复前实际运行 M9 `25a9d8c6cac0aa4d662fcbac55aea718b637b038`；修复后实际运行 H `1b474fd7f67cd8a33651f9cf56bc9694286b01ec`。后续文档提交仅复制本目录与截图，不能把 H 的测试或画面改标成文档提交的新运行。

原作固定版本 `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，`scripts/SilphCo11F.asm:225–233` 在最终对白后依次淡出、隐藏40个对象/显示6个对象、UpdateSprites、Delay3、淡入，最后才设置胜利旗标。名单见该文件73–147行，调色板和8帧等待见 `home/fade.asm`。本修复让两个胜利续程使用同一显式等待命令，按 BG/OBJ 各自原始像素索引应用调色板，并在黑屏的一次脚本更新中发布完整对象状态。

## 已实际完成

七阶段在 H 串行执行，全部通过：Data3、Boa1、完整 Core **--lib 3065**、Renderer1、完整 Agent58、App default2/debug-server2，共 **3132次通过、0失败**。其中12个新增专项测试执行14次；App两项在两个feature形态执行，属于组件测试。原名单、逐阶段命令、日志摘要哈希在 [provenance.json](provenance.json)。Agent严格未知语法检查和提交图逐字匹配保持通过；中途等待/snapshot、两个胜利入口和非胜利不应用均有回归。

正式画面对照分别从正常构建的 M9/H App lib 链接**同一份** [silph-witness.rs](silph-witness.rs)，不是 libtest。四个冷 Game 自然退出0；每端7928帧主段、3972帧冷加载/重入，合计23800次 input/update/draw/PCM pull、23804行状态、2022张运行时PNG，并逐帧保留完整 packed framebuffer/16色RGBA。独立读器核对了全部输入mask、状态、事件、packed哈希和全部PNG逐像素数据。

候选实际5823帧开始离场，5855帧发布46目标操作的**45位变化**，5890帧完成才设置旗标。物理初态是39shown+Saffron OBJ15原本OFF、6位市民hidden；不可写成46位都改变。黑屏19帧无残留文本/金钱框；5882帧颜色已正常，脚本仍阻塞至5890。两端均在7249帧发出SAVE音效、7318帧真正写盘；冷Continue加载各自主段写出的实际SRAM和实际伴随文件，再正常往返11F→7F→11F，37个固定停点、7F四敌/11F三敌隐藏、全部46位与钱包/HP/PP均通过，0新战斗/离场/保存。

## 同一绝对帧截图

[截图目录](../../screenshots/silph-rocket-departure-1072/) 内5831、5855、5866前后各一张，对应候选离场的+8淡出、+32黑屏批量更新、+43淡入。运行时PNG采样窗口4900–5800早于实际离场；这六张由**本次正式运行已记录的该帧完整 packed字节及实际RGBA表无损编码**，没有额外Game/update/draw，没有使用历史M8图。来源偏移、像素哈希和每图SHA见provenance；原运行PNG与大体积原始日志保留在审计目录。

## 从物理存档复现

本目录的 `include_str!` 文件名与实际witness完全相同。初始文件 [silph-before-giovanni.sav](silph-before-giovanni.sav) 是32KiB物理SRAM，不是运行态快照：11F(6,14)，Blastoise60满174HP，Surf/Bite/Withdraw/SkullBash，PP15/25/40/15，钱包10000；两名普通守卫已胜，Giovanni未胜。原始39shown+1OFF/6hidden名单不变。

在两个精确源版本分别执行正常构建 `cargo build -p pokered-app --lib --features debug-server --offline --locked -j1 -vv`，本次使用 dev/test opt-level=0、debug=0、incremental=0。**App rustc 的 `--extern` 不包含它自己**：由该条命令的 `--out-dir` 与 `-C extra-filename=...` 定位自身 `libpokered_app<extra-filename>.rlib`；其余六个库从同次 App rustc 的 `--extern name=path` 定位对应rlib，并核对实际文件。不要通配符任取缓存库；本次七个实际库的字节哈希见provenance。

以下展示本次确证的七个文件名及实际 `-L dependency/native` 路径，在本目录执行。其他构建环境必须按本次 App `-vv` 输出替换路径/文件名；本次 M9/H 的库文件名相同，但字节并不相同，须按各端哈希核验后立即链接。先冻结before ELF，再构建/链接after；after端将输出改为独立的 `after-witness`，不能覆盖before。

```sh
DEPS=/home/jerry/develop/open-pokered/.task-tmp/fidelity-1064-payday-target/debug/deps
NATIVE=/home/jerry/.local/share/starheir-build/sysroot/usr/lib/x86_64-linux-gnu
rustc --edition=2021 --crate-name silph_formal_ordinary_witness_1072 silph-witness.rs \
  -C debuginfo=0 -o ./before-witness \
  -L "dependency=$DEPS" -L "native=$NATIVE" \
  --extern "pokered_app=$DEPS/libpokered_app-a033ddfe954130fb.rlib" \
  --extern "pokered_core=$DEPS/libpokered_core-e07ae5bfb6097161.rlib" \
  --extern "pokered_renderer=$DEPS/libpokered_renderer-db429ded8bfb8bae.rlib" \
  --extern "dotzuki_engine=$DEPS/libdotzuki_engine-dd0c57d22d9f030e.rlib" \
  --extern "serde_json=$DEPS/libserde_json-2e61534cb1deef90.rlib" \
  --extern "pokered_audio=$DEPS/libpokered_audio-903466a71c39bea6.rlib" \
  --extern "pokered_data=$DEPS/libpokered_data-f594fe7b4773f2c9.rlib"
```

在本目录下，用已冻结的两个独立ELF按顺序运行（每个输出目录必须不存在）：

```sh
before-witness silph-before-giovanni.sav fixed-buttons7928.json before-main before main
after-witness silph-before-giovanni.sav fixed-buttons7928.json after-main after main
before-witness before-main/saved-artifacts/ordinary.sav reload-route-buttons3972.json before-reload before reload
after-witness after-main/saved-artifacts/ordinary.sav reload-route-buttons3972.json after-reload after reload
```

主段前7000输入逐帧不变，后928帧只追加普通 START、四次DOWN、SAVE确认、等待、B关闭；实际SAVE必须由真实120帧保存、SFX92完成和30帧等待后发生，固定中性预算不是猜音长。witness把真正写出的 `ordinary.sav` 及存在的 `ordinary.script_flags.json` 原字节放入 `saved-artifacts`，reload须保留相邻伴随文件，不能拿初始存档或JSON重建替代。冷加载前1460帧是原boot prefix，随后2512帧是预先冻结的普通方向路线；[literal-prefix-proof.json](literal-prefix-proof.json) 保留原逐帧相等证明。

每冷Game仅构造后首次update前设置seed42一次，后续只有普通物理输入。没有运行时setter、restore、warp、跳剧情、RPC、按候选结果调向量或seed。无设备音源每update后只拉取1470个混合f32样本，不额外推进sequencer；仅保留非零摘要，不宣称孤立声轨、设备听感或Direct音频。

此证据覆盖显式等待与实际显示/持久化，不声称原机CPU/OAM总帧或完整隐藏NPC/RNG历史一致。路线受原存档缺CardKey/3F门/Rival前置限制，只验证11F/7F正常重入，**未出楼到Saffron**。原M8发现记录保持历史身份。先前两个工具入口停止（symlink读器schema、ready SHA参数误抄）都在0Game/0rustc/Cargo时发生并保留原件，未覆盖为成功。
