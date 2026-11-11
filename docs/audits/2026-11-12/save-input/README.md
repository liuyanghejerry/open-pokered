# SAVE 松键由实际中断采样

原作 `pret/pokered@fbcf7d0e` 的 `ReadJoypad` 在 VBlank 中读取物理按键，
`HoldTextDisplayOpen` 经 `Joypad` 检查该次采样。复刻在 SAVE 恢复入口额外
直接发布物理按键，绕过中断；即使当前执行预算用尽，松开 A 仍能提前解除等待。
现在移除这条额外发布路径，沿用共享 IRQ 的真实采样。

回归测试先保持 A 到 `WaitForARelease`，在没有剩余执行预算时松键，确认采样和
等待状态均保持；后续真正的 IRQ 采样后才继续。基线为图鉴／资料重复计时修复
#454；该测试在修复前采样错误地从 1 变为 0，修复后通过。

## 实际输入与剩余差距

常青市普通 SRAM 冷启动，实际打开 START／SAVE，取消、取消时短按方向、NO
长按 A、YES 长按 A 各重复两次。每帧记录状态和图像，前后各 2,960 个实际帧；
坐标、朝向、队伍、SRAM、重开 START 和独立进程 CONTINUE 通过。四种输入的
图像均未变：取消回地图 t+39 与原作一致；NO 为 t+103、YES 为 t+623，原作
分别 t+104、t+624。YES 在 t+189 写档一次，其余分支不写。

另实际先进入图鉴、训练家资料或队伍，再进入 SAVE 选择 NO，保持 A 80 个
更新后松开。每组两遍，松键后各连续记录 40 帧，前后各 240 帧。三组回地图
从 t+22 改为 t+23，原作 t+24，**仍早一帧**。图鉴／资料组像素未变；队伍组
只有 t+22、t+23 的 人物区域不同，矩形为 `(65,60)–(79,76)`。前后截图取
相同松键后的实际帧，未移动时间轴、插帧或替换画面。

默认 GBA 从石英高原普通 SRAM 冷启动，六种前置情况（直接、图鉴、队伍、
背包、资料、设置）× 四种 SAVE 输入 × 两遍，共 48 次、69,600 个连续物理帧。
每次均实际回地图、重开 START、在独立模拟器 CONTINUE；两遍按键、图像及
SRAM 一致，仅 YES 写档，没有分配失败。直接 SAVE 的修复前后各 11,600
物理帧全部像素一致；取消／短按 t+282、NO t+711、YES t+2122 最终稳定。
GBA 物理帧与原作帧不是同一个时间单位，不据此声明时序等价。

应用 `debug-server` 全套 **1,132 通过、92 忽略**，桌面／默认 GBA 构建、七组
GBA 性能门限、新开局 m06（41.67 秒）通过。m06 只是早期推进回归；完整
主线、WASM 实机和物理音频等价未在本提交验证。SAVE 持续按 A 时的组合键
软复位缺失已另行复现，待独立修复。

## 复跑

直接 SAVE 使用 [已有 native／原作／GBA 脚本](../../2026-11-05/save-return/)，
native 和原作传入 `--record-all-frames`。前置菜单后的松键探针：

```sh
python3 replay-phase-native.py --binary /path/to/pokered-app --sha256 SHA --output /local/temp/native
python3 replay-phase-original.py --rom /path/to/pokered.gbc --symbols /path/to/pokered.sym --state /path/to/reference.state --output /local/temp/original
```

原作使用既有常青市准备状态；native 使用仓库常青市 SRAM。探针只通过按键
导航，状态查询和原作例程 hook 均只读。每种情况重复两次，连续保存每一个
实际帧。原始录制、运行报告、日志和校验清单只留在本地，验证后清理；不提交。

[相同松键时刻的原作／前／后截图](../../../screenshots/2026-11-12-save-input/party-native-before-after.png)。
