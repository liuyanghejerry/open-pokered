# 联机交换菜单差异 56–57

对照 pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`
`engine/link/cable_club.asm` 的 TradeCenter_SelectMon：

- `.chosePlayerMon` 打开横向 STATS / TRADE，默认 STATS。LEFT / RIGHT 分别选择对应动作；
  B 返回本方列表，STATS 依次显示两页再回本方列表，只有 TRADE 才提交所选索引。
- `.playerMonMenu_RightNotPressed` 前的切换及 `.enemyMonMenu_ANotPressed` 的 LEFT
  路径沿用当前菜单索引，并把超出对方队伍长度的索引限制到最后一只。

修复前，本方 A 直接提交交换，没有查看本方状态的入口；切换双方列表分别使用各自旧光标。
修复后补全上述操作，左右方向分别传递到共享运行时，重复按同一方向不反向切换菜单。
状态页沿用原作两页按键顺序及进入时一次叫声。操作菜单 B 只取消这层菜单，不发送交换取消。

## 验证

`game::link_stats_cry_fidelity_tests` 的实际双实例测试：

- 构造时满足已获得图鉴的入场条件；从合法交换中心位置、seed 0、实际 A 互动桌上的游戏机开始，通过真实 ChannelTransport
  握手、请求及对方接受到达列表；没有注入联机阶段或直接调用动作处理器。
- 本方默认 STATS、B 返回、查看自己的妙蛙种子及叫声、翻页退出、本方光标保持。
  重复 LEFT / RIGHT 分别保持 STATS / TRADE；双方选择 TRADE 后进入包含正确索引的确认阶段。
- 本方三只、对方两只时，索引 2 切换后限制为 1；返回沿用 1；对方把索引移到 0 后返回
  本方也为 0。客方镜像方向的越界索引也限制到己方最后一只。
- 核心全套 3118 项、debug app 库 163 项通过。完整默认 app 全套 422 项通过（17 项忽略），集成测试保留完成交换、再次交换、
  取消及双方退出的断言，按原作新增菜单选择 TRADE 后执行原有流程。
- 同输入截图捕获入口 `capture_own_mon_menu_and_stats`：构造存档、seed、输入均固定；
  菜单帧 363、查看状态帧 365、不同长度队伍切换后的光标帧 366。
  设置 `FIDELITY_LINK_CAPTURES` 后单独运行该忽略测试输出 PNG 和 frames.json。
  基线前后截图尚待音频 PR 合入 master 后归档；不能以只有后图代替 PR 比较图。

CANCEL 列表项、列表边界输入及其按键音仍在长期审计中，尚未由本批修复覆盖。
