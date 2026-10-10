# 队伍退出首帧扫描线（常青市场景，仍为暂定边界）

原作的常青市场景在首帧第16行变白，上方仍是当前队伍画面。原作上方16行与上一帧不同，说明不能冻结旧帧。现在绘制当前帧队伍图标，再从第16行起清白；下一帧全白。缓存键包含首帧边界，防止下一帧错误复用有色前缀。

相同原作 SRAM、实际 Continue/START/队伍/B输入的首帧。前为主分支31b1eda；后为修复分支；另列原作。沿用144批录制前景，当前Native四次重复录制。

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-raster-145/first-before.png)
![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-raster-145/first-after.png)
![原作](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-party-raster-145/first-original.png)

实际存档回归确认首帧保留有色前缀、下方白色，下一帧全白；82帧缓存/完整绘制相同。之前的短按丢弃/长按生效回归不变，原作/复刻的两个场景白色帧1..36共72帧解码RGB仍相同。核心2734项、应用201项单元测试通过，22项忽略。GBA发布构建、未修改基准的31项性能比较通过，10个原生/GBA源文件逐字相同。菜单快照原本明确拒绝，因此未扩充快照范围；恢复工作数据保留JSON默认边界以兼容上批数据。

**限制**：前端边界16暂取已测常青市场景。其他原作场景可能在15行及一行内改变调色板，仍需由共享CPU/PPU相位决定，不能称为全场景同步。不声称首帧所有原作/复刻像素相同，图标时钟及背景/NPC等仍待复核。冠军之路2F第二机关与已有合入门禁不变，PR保持草稿。
