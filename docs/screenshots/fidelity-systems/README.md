# 系统实际入口截图

最终after均来自冻结native APP，source `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`，SHA256 `ea717f1c2d6332f4608fc13194296cdb166ea20662d8d0f7c379bd17ba005e52`。保留项目原Fusion Pixel字体。11张after均160×144，逐张目检；既有before保持原始版本和字节，来源不统一替换成最终源码。

| 场景 | before | after |
|---|---|---|
| NPC真实精灵选择 | [72ff719](npc-selector-before.png) | [选择最后一只Abra](npc-selector-after.png) |
| 已连接后的柜员 | [72ff719](cable-reception-before.png) | [原Apply文本](cable-reception-after.png) |
| 柜员选房/取消 | 72ff719无此入口 | [3选项](cable-room-selection-after.png) / [原取消文本](cable-room-cancel-after.png) |
| 双TCP先后选招：peer | [8837a2a永久LinkWaiting](peer-colosseum-blocked-before.png) | [完整退战后全队治疗](peer-colosseum-party-after.png) |
| 双TCP终局镜像：host | [8837a2a永久LinkWaiting](host-colosseum-blocked-before.png) | [完整退战后全队治疗](host-colosseum-party-after.png) |
| 联机败方排普通回城 | [6fb9812回Pallet](peer-colosseum-blackout-before.png) | [继续120帧仍Colosseum](peer-colosseum-room-after.png) |
| 战前不治疗 | 原SYS13为战前错误治疗 | [host Surf2PP/Burn100HP](host-colosseum-entry-moves.png) / [peer Tackle3PP/Burn1HP](peer-colosseum-entry-moves.png) |
| 双方退战留房 | 联机终局曾无法正常退出 | [host竞技场](host-colosseum-room-after.png) / [peer竞技场](peer-colosseum-room-after.png) |
| 化石真实菜单B取消 | [c51209a无回应](fossil-cancel-before.png) | [两行原ComeAgain文本](fossil-cancel-after.png) |

NPC/SAVE/普通启动CONTINUE/stale-false companion/柜员合计15个状态检查，见 [完整证据](../../audits/2026-10-02-full-fidelity/systems-final-runtime.json)；实际双TCP单回合KO、100帧错峰选招、退战再120 neutral帧、全四只HP/status/PP恢复及金钱/EXP不变，见 [12阶段证据](../../audits/2026-10-02-full-fidelity/systems-colosseum-final.json)。两个JSON各保存最终build manifest、进程日志及逐图SHA256。

化石before/after是同一合成SaveData输入的真实Scientist1→filterBag→B→120帧；物品和复活标志保持。其 [before](../../audits/2026-10-02-full-fidelity/fossil-cancel-before.json) / [after](../../audits/2026-10-02-full-fidelity/fossil-cancel-after.json)记录源码、APP哈希、状态和逐图哈希；原作及GBA内存验证边界见 [补验](../../audits/2026-10-02-full-fidelity/systems-fossil-gba.md)。
