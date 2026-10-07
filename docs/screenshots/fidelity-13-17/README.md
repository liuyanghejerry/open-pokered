# 原作差异 13–17：支线规则与离场剧情

原作依据：pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`，
`scripts/NameRatersHouse.asm`、`scripts/GameCorner.asm`、
`scripts/IndigoPlateauLobby.asm`、`home/compare.asm` 和 `engine/math/bcd.asm`。

前：master `38c1e80`；后：本 PR。两次分别检出对应分支，使用各自的
headless debug-server 二进制、seed 0、speed 0 和相同的构造存档/输入。
截图均为实际共享运行时渲染，无截图后处理；构造存档不是新游戏通关证据。

| 场景 | 输入条件和采样点 | 前后共同帧数 |
|---|---|---:|
| name-rater | 另一位训练家的 Mr. Mime，选择后显示评价首屏 | 949 |
| coin-gift | 9990 代币，与赠送 20 代币的店员交谈 | 928 |
| coin-purchase | 9950 代币、¥5000，确认购买后停留文本 | 1000 |
| link-reception | 已获得图鉴、未联机，关闭联盟柜台欢迎文本后 | 928 |
| rocket-exit | Mewtwo L100 实际击败守卫，位于 (9,4)，关闭逃走台词后 8 帧 | 1816 |

`states-before.json` / `states-after.json` 保存截图同帧的完整调试状态。
`capture.py` 可重放上述场景：

```bash
LD_LIBRARY_PATH=/tmp/pokered-build-lib python3 docs/screenshots/fidelity-13-17/capture.py \
  --binary /path/to/pokered-app --label after --output /tmp/sidequest-captures
```

第 14 项原审计描述经源代码核对后细化：店员使用 `jr nc`，余额 ≥9990 拒绝；
绅士使用 `jr z`，仅余额 ==9990 拒绝。这一原作差异被分别保留。
购买及赠币使用原作 BCD 加法的 9999 封顶行为。例如 9950+50=9999，
支付 ¥1000；绅士在 9999 余额时仍给出领取台词并设置旗标，是原作的相等判断怪癖。

测试和新游戏 m01–m49 验证见 `docs/regression/fidelity-sidequests-13-17.json`。
