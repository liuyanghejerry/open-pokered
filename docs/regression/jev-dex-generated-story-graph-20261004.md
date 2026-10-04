# 同步大宅开关修复的生成故事图谱

PR107的9217289c在GitHub Coverage中失败于`semantics::gen::tests::graph_json_matches_committed_file`，不是Python策略测试。此前CinnabarIsland入口清除大宅开关的原生场景修复，尚未同步提交对应生成图谱。

使用仓库提供的`gen_event_graph`生成器更新`crates/pokered-data/story/graph.json`。唯一变化是新增：`script:CinnabarIsland:@load → flag:EVENT_MANSION_SWITCH_ON`，类型`clears`。边数3213→3214，没有删除边，没有改变任何赠送/对战物种来源。

修复后`cargo test --release -p pokered-agent --lib`全部51项通过，包括图谱逐字一致性、生成确定性和已知覆盖缺口检查。全部1231项scripts Python测试再次通过。没有画面或原生游戏行为变化，不增加新截图。

更新仅在隔离工作区生成。正在录像的第29段仍运行冻结的9217289c策略和a795d997原生二进制，工作区、图谱、存档、资源、模型选择和推进输入均不热更；最终合法NEW GAME仍未开始。远端CI的重新执行结果另行跟踪，不能把本地51项通过等同于整个远端工作流通过。
