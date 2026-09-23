#!/usr/bin/env python3
"""English variants of the blog figures used by blog/jev-playthrough-retro-2026-09.en.md.

Mirrors build_figures.py (same data and layout) with English labels, writing
`architecture-en`, `journey-en`, and `badge-order-en` (PNG + SVG). Copy the PNGs
into blog/images/ as jev-pipeline-architecture-en.png, jev-journey-82min-en.png,
and jev-badge-order-en.png.
"""
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
preferred=font_manager.FontProperties(family=['Arial','Helvetica','DejaVu Sans'])
plt.rcParams['font.family']=preferred.get_family()
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
ax.text(0,5,'From fixed-route regression to state-driven next steps',fontsize=23,fontweight='bold')
ax.text(0,4.55,'Original script: routes, preparation strategy, and acceptance points are hand-written',fontsize=14,color=GRAY)
for x,t,s in [(0,'m01 → m49','Fixed milestone order'),(3.45,'Generic skills + dedicated steps','Movement / menus / battles / triggers'),(6.9,'State assertions','Do the designated checkpoints hold?')]: box(ax,x,3.35,3.1,.9,t,s,GRAY)
arrow(ax,(3.12,3.8),(3.43,3.8));arrow(ax,(6.57,3.8),(6.88,3.8))
ax.text(0,2.9,'Jev: code provides viable candidates; both judgment layers share execution feedback',fontsize=14,color=GREEN)
for x,t,s in [(0,'Strategy-layer Jev','Story direction / resupply / training'),(3.45,'Action-layer Jev','Interaction target / menus / moves'),(6.9,'Code execution & verification','Input skills / budgets / real outcomes')]:box(ax,x,1.6,3.1,.9,t,s)
arrow(ax,(3.12,2.05),(3.43,2.05));arrow(ax,(6.57,2.05),(6.88,2.05))
box(ax,1.7,.15,6.6,.85,'Shared facts & memory','Condition-preserving script AST · live state · failure records\nflag / item / party changes',ORANGE)
arrow(ax,(8.45,1.57),(8.1,1.02),ORANGE);arrow(ax,(2,.99),(1.55,1.58),ORANGE);arrow(ax,(5,.99),(5,1.58),ORANGE)
ax.text(0,-.23,'White-box inputs; Jev judgments are not facts. Completion is confirmed by the Hall of Fame, credits, auto-save, and an independent CONTINUE.',fontsize=10,color=GRAY)
save(fig,'architecture-en')

fig,ax=plt.subplots(figsize=(13,5.4))
ms={m['objective']:m['elapsed_s']/60 for m in AUDIT['milestones']}
order=['beat-brock','beat-misty','beat-lt-surge','beat-koga','beat-erika','beat-sabrina','beat-blaine','beat-giovanni']
names=['Brock','Misty','Lt. Surge','Koga','Erika','Sabrina','Blaine','Giovanni']
times=[ms[m] for m in order];end=AUDIT['wall_s']/60
ax.axvspan(times[-1],end,fc='#eddfca',alpha=.75,zorder=0)
ax.step([0]+times+[end],[0]+list(range(1,9))+[8],where='post',color=GREEN,lw=2.8)
ax.scatter(times,range(1,9),color=GREEN,s=40,zorder=4)
for i,(t,n) in enumerate(zip(times,names)):
    ax.annotate(n,(t,i+1),xytext=(-7,12 if i!=6 else -27),textcoords='offset points',ha='center',fontsize=11)
ax.annotate('Full ending + independent load\n82 min 6 s',(end,8),xytext=(-8,-57),textcoords='offset points',ha='right',fontsize=12,fontweight='bold',color=GREEN)
ax.text(63,3.8,'About 36.7 minutes after badge 8:\nstill training, defeats, recovery, and ending verification',ha='center',fontsize=12,color=ORANGE)
ax.set(xlim=(-2,86),ylim=(-.25,9),xlabel='Wall-clock time of this run (minutes)',ylabel='Badges earned',yticks=range(9))
ax.set_title('The real progression of one continuous fresh save',loc='left',fontsize=22,fontweight='bold',pad=20)
ax.grid(axis='y',alpha=.15);ax.spines[['top','right']].set_visible(False)
fig.text(.125,-.025,'Source: 20260920-081028-seed42 audit. seed 42 / jev-1.13.0; not a cross-approach speed benchmark.',fontsize=10,color=GRAY)
save(fig,'journey-en')

fig,ax=plt.subplots(figsize=(13,3.8));ax.set(xlim=(-1.75,8),ylim=(-.75,2.2));ax.axis('off')
ax.text(-1.6,1.95,'Same endpoint, different progression orders',fontsize=22,fontweight='bold')
for y,label,seq in [(1.1,'Original script',['Brock','Misty','Lt. Surge','Erika','Koga','Sabrina','Blaine','Giovanni']),(0,'Jev (this run)',['Brock','Misty','Lt. Surge','Koga','Erika','Sabrina','Blaine','Giovanni'])]:
    ax.text(-1.6,y,label,fontsize=14,va='center',fontweight='bold',color=GRAY if y else GREEN)
    for i,name in enumerate(seq):
        c=ORANGE if i in (3,4) else (GRAY if y else GREEN)
        ax.scatter(i,y,s=2400,facecolor='#fff',edgecolor=c,linewidth=2,zorder=2)
        ax.text(i,y,name,ha='center',va='center',fontsize=10,color=c,zorder=3,linespacing=1.1)
        if i<7: arrow(ax,(i+.34,y),(i+.66,y),GRAY)
ax.text(-1.6,-.65,'Evidence of a reordered route only — it does not mean Jev covered every side quest, item, and assertion of m01–m49.',fontsize=11,color=GRAY)
save(fig,'badge-order-en')
print('Generated architecture-en, journey-en, badge-order-en (PNG + SVG)')
