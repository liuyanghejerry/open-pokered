# 宝可梦之笛与 TM29 领取确认

领取音效结束后保留回执，等待玩家确认；富士老人完成使用说明后设置领取标记，超能力者在音效结束后设置标记，再等待外层确认。满包和重复交谈也保留原作确认流程。

当前源码为 `d1239c9d`，已合并 master `d124973c`。当前基底的完整本地检查、4 组各两次准备存档输入、8 次保存后的独立 CONTINUE，以及两张地图各两次前后截图均通过。当前截图与已目视检查的四张 PNG 逐字节相同。详见 `current-base-build.json.gz`、`current-base-recordings.json.gz` 和 `verification.json` 的 `latest_base_validation`。

完整 NEW GAME → m49 → 冠军／名人堂／字幕／结局存档 → 独立 CONTINUE 的 8,046 秒通过证据属于较早的独立冻结候选 `7a8e94da`，其 crates/Cargo 与实现提交 `13596a94` 相同。该证据保留原始源码与二进制身份；更新 master 后的组合构建为 `d85ba8e2`，本目录没有把旧通关记录改标为这个新构建。PR429 的钓竿修改有其独立完整通关记录。

准备存档对照、完整新游戏以及 CI 分别记录。原始存档、压缩录像记录、完整通关记录和结束存档均在本目录；截图位于 `docs/screenshots/fuji-psychic-gift-confirmation/`。
