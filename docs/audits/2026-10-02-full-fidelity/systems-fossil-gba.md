# 化石研究所 GBA 内存与取消补验

保留项目原字体的冻结版本 `c51209a492f804f58486607919c9ba0c62eb2eb7` 在真实 mGBA 0.10.5 压力流程中仍发生分配失败。图鉴302项、地图248张、包裹交付和16个图鉴评价分支已通过；满6只队伍、12×20个箱精灵、50×6条名人堂记录且昵称80B的合成压力存档，随后在化石研究所 Scientist1 交互入口失败。入口连续可用堆28720 B、剩余栈13224 B；完整 `talkScientist1` 序列化AST为17490 B，即使背包没有化石，也会解码全部交付/领取分支并被解释器深拷贝。基线证据由战斗代理归档于 [GBA 内存验证](validation/final-font-preserved-gba.md)，此处不将 native 测试通过当作修后 GBA 压力实测通过。

修复源码 `537eda7` 与 `eebde2f` 在构建时从原 `.scene` AST 提取小片段，仅对嵌入式函数启用延迟选择。完整AST保留为宿主测试语义参照与异常多种待复活标志的回退；磁盘加载与可选Boa路径仍执行完整场景。

| 片段 | 实际构建长度 | 行为 |
|---|---:|---|
| entry | 1371 B | 原博士介绍、原hasItem判断、真实filterBag菜单或没有化石文本 |
| fossil_dome / helix / amber | 2588 / 2593 / 2593 B | 菜单返回后才加载对应描述、YES/NO、消费化石和复活标志 |
| still | 303 B | 尚未完成复活，仅提示出去走走 |
| ready_kabuto / omanyte / aerodactyl | 2140 / 2142 / 2154 B | 先置HANDING_OVER，再尝试Lv30交付；失败保留，成功清除原六项标志 |
| ready_none | 199 B | 原作场景没有任何物种标志时仅登记HANDING_OVER |
| cancel / noop | 242 / 2 B | 真实空字符串取消显示原文；未知或非文本调试响应保持无动作 |

entry在实际filterBag等待时释放自己的解释器帧，菜单返回的 `CommandResult::Text` 决定三个化石续段；没有按背包里的第一种化石提前选分支。等待和新格式lazy菜单快照的实际恢复都保留交互阶段。旧格式缺少新增marker字段仅验证serde反序列化，不声明已实测旧完整VM程序计数器的中途续跑；普通SRAM不保存VM执行栈或NativeScriptEngineSnapshot。多个REVIVING标志仍回退完整场景，以保留三条独立条件与失败后继续尝试下一物种的既有语义；这类异常状态未声明满足GBA的内存余量。

另一个实际入口差异是B取消后没有博士回应。原作 `engine/events/cinnabar_lab.asm:30–31` 从B进入 `:70–73` 的 `.cancelledGivingFossil`，打印 `_CinnabarLabFossilRoomScientist1ComeAgainText`；[原文](https://github.com/pret/pokered/blob/fbcf7d0/text/CinnabarLabFossilRoom.asm)`:75–78` 为两行 `Aiyah! You come\nagain!`。完整scene及lazy取消续段同时补该回应；取消不消费化石、不开始复活。基线冻结程序实际走 OldAmber×1 → Scientist1 → filterBag → B → 120帧，仍有物品而没有回应：[状态与输入](fossil-cancel-before.json)、[before图](../../screenshots/fidelity-systems/fossil-cancel-before.png)。after已在最终冻结程序 `eebde2fc3a4f78c38b4c7b5d49e0db2b830cb235`（APP SHA256 `ea717f1c2d6332f4608fc13194296cdb166ea20662d8d0f7c379bd17ba005e52`）上用 [capture-fossil-cancel.py](capture-fossil-cancel.py) 同输入补验PASS：[完整状态](fossil-cancel-after.json)、[after图](../../screenshots/fidelity-systems/fossil-cancel-after.png)。实际B后推进120帧，屏上两行原文、物品和flags不变。debug状态的dialogue摘要会合并空格；原script effect保留硬换行，截图验证最终两行。

正式 native 验证已完成：新4项 `fossil_lab_` 测试通过，覆盖EN/ZH的160组入口输入与64组ready输入、3物种失败后成功重试、真实菜单返回值、100帧等待、菜单快照恢复、旧快照缺字段的serde反序列化读取及11片段均小于3000 B。另有已有化石满箱重试测试1项、agent全量53项通过。B取消和三个NO分支的测试使用独立原作两行文本预期，完整/lazy相同不作为原作正确的唯一依据。重新生成事件图仍为3278条边，committed graph无变化；覆盖检查通过。具体命令、结果及片段哈希见 [native验证记录](fossil-lazy-native-validation.json)。

GBA压力修后验收需用新冻结源码重建ROM并重新运行完整CI场景；原满队/满箱/名人堂压力数据、4096 B堆/栈门槛与计数均未调整。
