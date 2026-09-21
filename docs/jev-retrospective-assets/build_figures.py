#!/usr/bin/env python3
"""Rebuild figures from committed audit data (requires matplotlib)."""
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch, FancyArrowPatch

ROOT=Path(__file__).resolve().parents[2]
OUT=Path(__file__).resolve().parent
AUDIT=json.loads((ROOT/'docs/audits/2026-09-20/jev-autonomous-seed42.json').read_text())
font=Path('/Library/Fonts/Arial Unicode.ttf')
if font.exists():
    font_manager.fontManager.addfont(str(font))
    plt.rcParams['font.family']=font_manager.FontProperties(fname=str(font)).get_name()
plt.rcParams.update({'font.size':12,'axes.unicode_minus':False,'figure.facecolor':'#f6f4ee','axes.facecolor':'#f6f4ee','text.color':'#24312b','axes.labelcolor':'#24312b','xtick.color':'#52615a','ytick.color':'#52615a','savefig.facecolor':'#f6f4ee','svg.fonttype':'path'})
GREEN='#237451';ORANGE='#b65a2c';GRAY='#62726c';INK='#24312b'
def save(fig,name):
    fig.savefig(OUT/(name+'.png'),dpi=160,bbox_inches='tight')
    fig.savefig(OUT/(name+'.svg'),bbox_inches='tight')
    plt.close(fig)

def box(ax,x,y,w,h,title,sub,color=GREEN):
    ax.add_patch(FancyBboxPatch((x,y),w,h,boxstyle='round,pad=0.015,rounding_size=0.03',fc='#ffffff',ec=color,lw=1.5))
    ax.text(x+w/2,y+h*.67,title,ha='center',va='center',fontsize=15,fontweight='bold',color=color)
    ax.text(x+w/2,y+h*.27,sub,ha='center',va='center',fontsize=11,linespacing=1.6)
def arrow(ax,a,b,color=GRAY):
    ax.add_patch(FancyArrowPatch(a,b,arrowstyle='-|>',mutation_scale=16,color=color,lw=1.7))
fig,ax=plt.subplots(figsize=(13,7));ax.set(xlim=(-.15,10.15),ylim=(0,5.2));ax.axis('off')
ax.text(0,5,'从指定路线回归，到按状态选择下一步',fontsize=23,fontweight='bold')
ax.text(0,4.55,'原脚本：路线、准备策略与验收点由人预先编写',fontsize=14,color=GRAY)
for x,t,s in [(0,'m01 → m49','固定里程碑顺序'),(3.45,'通用技能 + 专用步骤','移动 / 菜单 / 战斗 / 机关'),(6.9,'状态断言','指定检查点是否满足')]: box(ax,x,3.35,3.1,.9,t,s,GRAY)
arrow(ax,(3.12,3.8),(3.43,3.8));arrow(ax,(6.57,3.8),(6.88,3.8))
ax.text(0,2.9,'Jev：代码提供可行候选，两层判断共同利用执行反馈',fontsize=14,color=GREEN)
for x,t,s in [(0,'策略层 Jev','剧情方向 / 补给 / 训练'),(3.45,'动作层 Jev','交互对象 / 菜单 / 招式'),(6.9,'代码执行与验证','按键技能 / 预算 / 真实结果')]:box(ax,x,1.6,3.1,.9,t,s)
arrow(ax,(3.12,2.05),(3.43,2.05));arrow(ax,(6.57,2.05),(6.88,2.05))
box(ax,1.7,.15,6.6,.85,'共享事实与记忆','保留条件的脚本 AST · 实时状态 · 失败记录 · flag / 物品 / 队伍变化',ORANGE)
arrow(ax,(8.45,1.57),(8.1,1.02),ORANGE);arrow(ax,(2,.99),(1.55,1.58),ORANGE);arrow(ax,(5,.99),(5,1.58),ORANGE)
ax.text(0,-.23,'白盒输入；Jev 判断不等于事实。完成由名人堂、片尾、自动存档及独立 CONTINUE 共同确认。',fontsize=10,color=GRAY)
save(fig,'architecture')

fig,ax=plt.subplots(figsize=(13,5.4))
ms={m['objective']:m['elapsed_s']/60 for m in AUDIT['milestones']}
order=['beat-brock','beat-misty','beat-lt-surge','beat-koga','beat-erika','beat-sabrina','beat-blaine','beat-giovanni']
names=['小刚','小霞','马志士','阿桔','莉佳','娜姿','夏伯','坂木']
times=[ms[m] for m in order];end=AUDIT['wall_s']/60
ax.axvspan(times[-1],end,fc='#eddfca',alpha=.75,zorder=0)
ax.step([0]+times+[end],[0]+list(range(1,9))+[8],where='post',color=GREEN,lw=2.8)
ax.scatter(times,range(1,9),color=GREEN,s=40,zorder=4)
for i,(t,n) in enumerate(zip(times,names)):
    ax.annotate(n,(t,i+1),xytext=(-7,12 if i!=6 else -27),textcoords='offset points',ha='center',fontsize=11)
ax.annotate('完整结局 + 独立读档\n82 分 6 秒',(end,8),xytext=(-8,-57),textcoords='offset points',ha='right',fontsize=12,fontweight='bold',color=GREEN)
ax.text(63,3.8,'八徽章后约 36.7 分钟\n仍有训练、败退、恢复与结局验证',ha='center',fontsize=12,color=ORANGE)
ax.set(xlim=(-2,86),ylim=(-.25,9),xlabel='本次运行的墙钟时间（分钟）',ylabel='已获得徽章数',yticks=range(9))
ax.set_title('一次连续新开档的真实进程',loc='left',fontsize=22,fontweight='bold',pad=20)
ax.grid(axis='y',alpha=.15);ax.spines[['top','right']].set_visible(False)
fig.text(.125,-.025,'来源：20260920-081028-seed42 审计。seed 42 / jev-1.13.0；不是跨方案的速度基准。',fontsize=10,color=GRAY)
save(fig,'journey')

fig,axs=plt.subplots(1,3,figsize=(13,4.8),gridspec_kw={'width_ratios':[1,1,1.35]})
for ax,title,values,unit in [(axs[0],'Jev 判断次数',[383,2033],'次'),(axs[1],'模型输入 tokens',[2189612/1e6,7799443/1e6],'百万')]:
    ax.bar(['策略','动作'],values,color=[GRAY,GREEN],width=.57)
    for i,v in enumerate(values):ax.text(i,v*1.035,f'{v:.2f}' if unit=='百万' else str(v),ha='center',fontsize=14,fontweight='bold')
    ax.set_title(title,loc='left',fontsize=16,pad=18);ax.set_ylabel(unit);ax.set_ylim(0,max(values)*1.25)
    ax.spines[['top','right']].set_visible(False);ax.grid(axis='y',alpha=.12)
vals=list(AUDIT['defeats_by_map'].values());axs[2].barh(['小霞','科拿','渡'],vals,color=ORANGE,height=.5);axs[2].invert_yaxis()
for i,v in enumerate(vals):axs[2].text(v+.15,i,str(v),va='center',fontsize=14,fontweight='bold')
axs[2].set(xlim=(0,12),xlabel='战败次数');axs[2].set_title('16 次真实战败',loc='left',fontsize=16,pad=18);axs[2].spines[['top','right']].set_visible(False)
fig.suptitle('通关已实现，成本与战斗策略仍需优化',x=.05,ha='left',fontsize=22,fontweight='bold')
fig.subplots_adjust(top=.77,wspace=.5,bottom=.19)
fig.text(.05,.015,'输出 137,967 tokens；输入合计 9,989,055。此图描述单次成功运行，不代表平均成本或稳定成功率。',fontsize=10,color=GRAY)
save(fig,'cost-and-defeats')

fig,ax=plt.subplots(figsize=(13,3.8));ax.set(xlim=(-1.75,8),ylim=(-.75,2.2));ax.axis('off')
ax.text(-1.6,1.95,'相同终点，可以产生不同的推进顺序',fontsize=22,fontweight='bold')
for y,label,seq in [(1.1,'原脚本',['小刚','小霞','马志士','莉佳','阿桔','娜姿','夏伯','坂木']),(0,'Jev 本次',['小刚','小霞','马志士','阿桔','莉佳','娜姿','夏伯','坂木'])]:
    ax.text(-1.6,y,label,fontsize=14,va='center',fontweight='bold',color=GRAY if y else GREEN)
    for i,name in enumerate(seq):
        c=ORANGE if i in (3,4) else (GRAY if y else GREEN)
        ax.scatter(i,y,s=1300,facecolor='#fff',edgecolor=c,linewidth=2,zorder=2)
        ax.text(i,y,name,ha='center',va='center',fontsize=11,color=c,zorder=3)
        if i<7: arrow(ax,(i+.24,y),(i+.76,y),GRAY)
ax.text(-1.6,-.65,'只是顺序变化的实证：不意味着 Jev 覆盖了 m01–m49 的所有支线、道具和断言。',fontsize=11,color=GRAY)
save(fig,'badge-order')
print('Generated architecture, journey, cost-and-defeats, badge-order (PNG + SVG)')
