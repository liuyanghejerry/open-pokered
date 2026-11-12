# SAVE 等待松键时的软复位

SAVE 选 NO／YES 后持续按住 A，会进入 `HoldTextDisplayOpen`。原作在这里
调用 `_Joypad`，精确匹配 A+B+SELECT+START 后进入 `TrySoftReset`。复刻的
共享 IRQ 接管时跳过全局复位检查，等待门本身却没有补上检查，因此一直不复位。

修复在这一次真实前台检查中读取 IRQ 采样；匹配四键后，执行原作 `DelayFrame`
及其后的 JOYP 取消选择、计数递减和分支。只在实际中断返回后递减，耗尽执行
预算会保留未完成指令；组合键不更新普通按键历史，其他按键也不重置累计计数。
计数完成后使用现有桌面重置／GBA BIOS 重启入口。其他恢复操作不检查此工作项。

原作依据：`pret/pokered@fbcf7d0e` 的 `engine/joypad.asm`、
`home/text_script.asm::HoldTextDisplayOpen` 和 `home/vblank.asm::DelayFrame`。

## 实际验证

- 应用回归先在旧实现复现等待门无法复位，再覆盖 NO、YES、额外方向键和复位
  不重复写档。另用合成总线验证中断返回／执行预算边界；它不代表实际 IRQ 耗时。
- 默认 GBA 从石英高原普通 SRAM 冷启动。START 对照、SAVE NO、SAVE YES
  各两遍，按住组合键连续记录 512 个物理帧，共 3,072 帧。均实际重启到语言
  选择页，再通过正常按键 CONTINUE 回到地图；复位和 CONTINUE 都没有改写
  已持久化字节。旧实现的 NO 等待门两遍均持续 512 帧不复位。
- 原作从既有常青市准备状态，通过实际按键进入同类三种场景，各两遍；六次
  都在 t+16 调用 `SoftReset`。场景存档地点不同，此处验证功能和调用顺序。
- 桌面前置图鉴／资料／队伍后执行 SAVE NO，松键后的 240 个实际帧状态及图像
  与父提交完全一致。桌面组合键由 `InputState` 集成测试覆盖，没有桌面多键 UI
  的逐帧录制。

| 默认 GBA：按下组合键后首次画面变化 | 修复前 | 修复后 |
| --- | --- | --- |
| 普通 START | t+17 | t+17 |
| SAVE NO 等待松键 | 512 帧内没有 | t+111 |
| SAVE YES 等待松键 | 未单独录制旧版 | t+122 |

这些 GBA 物理帧不是原作源帧，**没有实现完整时序／物理音频等价**。

普通 SAVE 的六种前置菜单 × 四种输入 × 两遍，共 48 次、69,600 连续物理帧
全部通过；实际回地图、重开 START、独立 CONTINUE 和 SRAM 校验通过。两遍
输入、图像和 SRAM 一致，只有 YES 写档。全部 48 份 SRAM 与父提交逐字节
一致，桌面上述松键状态及像素未变。

默认 GBA 的资料／图鉴前置稳定时间与父提交一致。背包／设置前置的取消和短按
场景最终稳定仍晚 1 个物理帧，其他场景稳定时间相同；没有据此宣称动画完全等价。

应用全套 **1,136 通过、92 忽略**，桌面／默认 GBA 构建、七组 GBA 性能门限、
新开局 m06（41.72 秒）通过。未以 m06 代替完整主线验证。

另行扫描发现：在 SAVE 确认框短按四键组合后松开，原作留在确认框，复刻会误按
B 取消回地图；未发生误写档。这是等待门之前的输入分发问题，尚待独立修复。

## 复跑

```sh
python3 replay-gba.py 1 --probe save --choice no --expect-reset --rom /path/to/default.gba --sha256 SHA --bridge /path/to/mgba-bridge.so --output /local/temp/gba-no-1
python3 replay-original.py --rom /path/to/pokered.gbc --symbols /path/to/pokered.sym --state /path/to/reference.state --output /local/temp/original
```

GBA 使用现有的 `party-return/indigo/fixture.sav`，通过 `--choice yes` 验证真正
写档后的复位，通过 `--probe start` 做普通菜单对照。脚本只输入按键、读回 SRAM
和录制实际帧。正常 SAVE 和桌面松键复用相邻审计目录的生成器。
原始录制、报告和日志保存在本地，验证后清理，不进入提交。

[相同按键／物理帧的前后截图](../../../screenshots/2026-11-12-save-soft-reset/save-no-gba-before-after.png)。
