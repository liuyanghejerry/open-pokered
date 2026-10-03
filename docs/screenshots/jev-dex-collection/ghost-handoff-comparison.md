# 脚本幽灵战后的旧方向步行

- 前：master `0f94a6fa`，二进制 SHA256
  `5136b5b943fc30a93c8dd72b65a0adca4e202cbc1a29c27b11a7d9296731165d`。
- 后：脚本战挂起时清除旧 movement_state／walk_counter，二进制 SHA256
  `2eab8a49e38cb78e5207e49fcf8c3273962b38993169399a7501dee363ed091e`。
- 两边相同 seed42、speed0、共享同一锁定 gfx，原生160×144截图；未编辑图片。
- 只用于隔离视觉回归：`--skip-intro --warp PokemonTower6F,11,16`，
  `give_pokemon Charizard Lv54`。不属于正式图鉴、正式存档或正式录像。
- 起始观察第100帧，从 `(11,16)` 持LEFT9帧触发真实坐标剧情、
  无SilphScope的Marowak Lv30幽灵战；相同输入到第424帧 PlayerMenu。
- 都从真实菜单选择RUN，第464帧返回overworld，再推进100帧无按键，
  第564帧 `capture_frame`；两边幽灵胜利旗标都保持false。
- 前：残存LEFT Walking先于脚本RIGHT推回完成，踏入楼梯并到7F `(9,16)`。
- 后：无旧LEFT步行；实际RIGHT推回，留在6F `(11,16)`，不能把RUN当作胜利。

同一辅助脚本的逐条请求、原生状态及日志保存在持久目录
`.artifacts/checkpoint-safety-20261003/ghost-handoff-visual-master-b/` 与
`ghost-handoff-visual-after/`；辅助脚本为 `ghost-handoff-visual.py`。
原始33种正常CONTINUE诊断另行保留，不与上述种子截图混算。

核心回归覆盖真实6F场景的 `ran`、`caught`、`win`、`fled` 四种返回；
前两者仍推回且不设置胜利，后两者保留原场景的胜利／Poké Doll语义。
没有改写Ghost Marowak场景或把普通逃跑伪造为击败。
