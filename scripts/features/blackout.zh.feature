# language: zh-CN
# 中文关键字演示：步骤词典本身是英文的（bdd_steps.py 的正则只匹配
# 英文步骤文本），这里证明解析器接受 zh-CN 的 功能/场景/假如/当/那么。
功能: 战斗白屏
  场景: 全军覆没后回到家中且队伍回满
    假如 a booted game
    并且 the player has a Rattata at level 2
    当 a wild Beedrill at level 30 attacks
    并且 the player fights until the battle ends
    并且 the whiteout settles
    那么 the player is on PalletTown
    并且 the party is fully healed
