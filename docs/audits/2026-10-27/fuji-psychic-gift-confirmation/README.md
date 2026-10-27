# 宝可梦之笛与 TM29 领取确认

富士老人发放笛子后，领取文字应保留到玩家确认，再继续使用说明，最后设置领取标记。超能力者的 TM29 音效结束后设置标记，并等待外层确认。满包、重复交谈和长按确认也使用原作对应的对话边界。

`verification.json` 记录实现提交、冻结构建、检查结果和证据哈希。四张前后截图使用相同原作存档、相同初始画面和绝对帧；两侧的对话确认前缀不同，各自重复两遍。字体保持 Fusion Pixel。

`recordings.json.gz` 和四个准备存档记录成功与满包各两遍，以及八次保存后的独立进程 CONTINUE。它们与完整新游戏验证分开报告：`fresh-run.json.gz`、`fresh-milestones.json.gz`、`fresh-driver.log.gz` 和 `first-clear.sav.gz` 对应从 NEW GAME 到冠军、名人堂、字幕、结局存档及独立进程 CONTINUE 的完整通过记录。
