#!/usr/bin/env python3
"""Build a seekable comparison with synchronized Jev evidence and reading holds."""
import html
import json
import subprocess
from pathlib import Path

GUIDES = [
 '观察初始精灵、目标选择与领取图鉴的操作。Jev 的方向来自策略层选择，具体操作由动作层和通用技能完成。',
 '观察首次挑战的准备与失败后的变化。大盘中的候选是当时提供给模型的选择，不代表所有可能策略。',
 '比较队伍属性、训练与重试。取得同一枚徽章，并不意味着采用了同样的准备方式。',
 '观察船票、秘传机和第三道馆如何串联。左侧按里程碑调度，右侧按当前前置条件选择目标。',
 '第四、第五道馆的顺序不同，因此合并成“五枚徽章”阶段。两条录像都保持原来的时间顺序。',
 '比较主线推进、队伍扩充和复杂机关。到达相同终点，不等于覆盖相同地图和支线。',
 '观察进入联盟之前是否备齐技能与物资。Jev 本轮在八徽章后折返寻找金牙、准备怪力。',
 '比较连续战斗中的血量、招式与补给。关键节点将暂停 8 秒，展示真实候选、结果和复盘注解。',
 '两路均实际播放名人堂与片尾。最终成功还需要确认自然存档可在独立进程继续游戏。',
 '这一章是各自新启动的独立进程：读取结局自动存档，并核验真新镇、八徽章与名人堂记录。',
]


def compose(chapters, recordings, project):
    data=json.loads((project/'full-run/jev-dashboard.json').read_text())
    target=project/'full-run'
    parts=[];cursor=0
    for i,c in enumerate(chapters):
        c['start']=cursor
        side=c['sides']['jev'];span=c['duration']
        notes=[n for n in data['notes'] if side['source_start']<=n['source_s']<side['source_end']]
        positions=[((n['source_s']-side['source_start'])/c['rate'],n) for n in notes]
        before=0
        for at,note in positions+[(span,None)]:
            if at>before:
                parts.append({'chapter':i,'start':cursor,'duration':at-before,'normal_start':before,'hold':False})
                cursor+=at-before
            if note:
                parts.append({'chapter':i,'start':cursor,'duration':note['hold_s'],'normal_start':at,'hold':True,'note':note})
                cursor+=note['hold_s']
            before=at
        c['overview_duration']=cursor-c['start']
    content=[]
    for number,p in enumerate(parts):
        c=chapters[p['chapter']];t=p['start'];duration=p['duration'];normal=p['normal_start']
        for name,side in c['sides'].items():
            position,track=('left',1) if name=='script' else ('right',2)
            base=f'{name}-part-{number}'
            if p['hold']:
                second=min(side['source_end']-1/60,side['source_start']+normal*c['rate'])
                if normal >= side['duration']:
                    second=side['still_source_s']
                still=target/f'{base}-hold.png'
                source=next(r['file'] for r in recordings if r['controller']==name)
                subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-nostdin','-y','-ss',f'{second:.9f}',
                                '-i',str(target/source),'-frames:v','1','-update','1',str(still)],check=True)
                content.append(f'<img id="{base}" class="clip screen {position}" src="full-run/{still.name}" data-start="{t}" data-duration="{duration}" data-track-index="{track}" alt="关键决策时的真实画面">')
            else:
                moving=max(0,min(duration,side['duration']-normal))
                if moving>0:
                    content.append(f'<video id="{base}" class="clip screen {position}" src="full-run/{side["file"]}" data-start="{t}" data-duration="{moving}" data-media-start="{normal}" data-track-index="{track}" muted playsinline></video>')
                if moving<duration:
                    content.append(f'<img id="{base}-end" class="clip screen {position}" src="full-run/{side["still"]}" data-start="{t+moving}" data-duration="{duration-moving}" data-track-index="{track}" alt="本章终态定格">')
    meta={'chapters':chapters,'parts':parts,'recordings':recordings,'guides':GUIDES}
    page='''<!DOCTYPE html><html lang="zh-CN"><head><meta charset="UTF-8"><meta name="viewport" content="width=1920,height=1080">
<title>完整通关对比 · Jev 决策大盘</title><script src="video-assets/gsap.min.js"></script>
<script src="full-run/jev-dashboard-data.js"></script><script src="jev-dashboard.js"></script><link rel="stylesheet" href="jev-dashboard.css">
<style>
@font-face{font-family:Evidence;src:url('video-assets/evidence-font.ttf') format('truetype')}
*{box-sizing:border-box}html,body{margin:0;width:100%;height:100%;overflow:hidden;background:#f6f4ee;color:#24312b;font-family:Evidence,sans-serif}
#root{position:relative;width:100%;height:100%;background:#f6f4ee}h1{position:absolute;left:64px;top:34px;margin:0;font-size:43px}
#chapter-label{position:absolute;left:64px;top:101px;font-size:27px;width:1040px;line-height:1.3}
.label{position:absolute;top:168px;font-size:27px;font-weight:700}.left{left:64px}.right{left:624px}.right.label{color:#237451}
.screen{position:absolute;top:212px;width:480px;height:432px;object-fit:contain;image-rendering:pixelated;background:#e1e4d9}
.source-status{position:absolute;top:649px;font-size:18px;width:480px;line-height:1.4;height:30px}
#rate{position:absolute;left:64px;top:1037px;font-size:16px;color:#52615a}
#provenance{position:absolute;left:64px;top:685px;width:480px;height:225px;border:1px solid #bdc9bf;border-radius:8px;background:#edf0e8;padding:11px 14px}
#provenance h2{margin:0 0 5px;font-size:18px}
#provenance p{margin:0 0 6px;font-size:15px;line-height:1.45}
#inputs-caption{position:absolute;left:624px;top:685px;width:480px;font-size:15px;color:#52615a}
#model-inputs{position:absolute;left:624px;top:709px;width:480px;height:206px;margin:0;display:flex;flex-direction:column;gap:8px}
#model-inputs .jd-input-card{padding:7px 12px;overflow:hidden;flex:1;min-height:0}
#model-inputs .jd-input-title{display:inline;font-size:15px}#model-inputs .jd-input-meta{display:inline;font-size:11px;margin:0 0 4px 8px}
#model-inputs .jd-input-rows{font-size:14px;line-height:1.35;gap:2px 8px;margin-top:4px}
#model-inputs dd{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#annotation{position:absolute;left:64px;top:922px;width:1040px;height:102px;border-top:3px solid #237451;padding-top:8px}
#annotation-label{display:inline;font-size:16px;color:#52615a}#annotation-title{display:inline;font-size:23px;font-weight:700;margin-left:12px}#annotation-body{font-size:21px;line-height:1.4;margin-top:5px}
#dashboard{position:absolute;left:1160px;top:130px;width:696px;height:895px;padding:26px}
#dashboard .jd-party{min-height:110px}#dashboard .jd-location{font-size:24px;margin:9px 0 13px}
#dashboard .jd-strategy{position:absolute;left:26px;right:26px;top:243px;padding-top:12px}
#dashboard .jd-goal{min-height:45px;font-size:26px}#dashboard .jd-candidates{min-height:140px}
#dashboard .jd-option{margin:7px 0 9px}#dashboard .jd-option-label{font-size:19px}
#dashboard .jd-action-section{position:absolute;left:26px;right:26px;top:535px;margin:0;padding-top:10px}
#dashboard .jd-command-section{position:absolute;left:26px;right:26px;top:622px;margin:0;padding-top:10px}
#dashboard .jd-action{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#dashboard .jd-command{font-size:21px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#dashboard .jd-result-section{position:absolute;left:26px;right:26px;top:712px;margin:0;padding-top:10px}
#dashboard .jd-counts{position:absolute;left:26px;right:26px;top:821px;margin:0;padding-top:12px;font-size:18px}
#dashboard .jd-clock{position:absolute;left:26px;right:26px;top:866px;font-size:16px;margin:0}
.foot{position:absolute;left:1160px;top:1041px;margin:0;font-size:14px;color:#52615a}
</style></head><body><div id="root" data-composition-id="main" data-start="0" data-duration="__DURATION__" data-width="1920" data-height="1080">
<h1>脚本自动化 × Jev 自主探索 · 完整通关与决策大盘</h1><div id="chapter-label"></div>
<div class="label left">脚本（无模型参与）</div><div class="label right">策略层 + 动作层 Jev</div>
__CONTENT__
<div id="script-status" class="source-status left"></div><div id="jev-status" class="source-status right"></div><div id="rate"></div>
<div id="provenance"><h2>左侧录像 · 纯脚本运行</h2><p>从 NEW GAME 到名人堂完全由预写脚本驱动，全程没有任何模型参与，作为右侧 Jev 录像的对照基线。</p><p>因此左下方没有模型输入卡片——右侧的「当次输入」快照只来自 Jev 录像。</p><p>两路使用相同引擎与 seed 42，按共同剧情节点分章对照。</p></div>
<div id="inputs-caption">策略层 / 动作层 · 当次输入快照 — 仅对应右侧 Jev 录像</div>
<div id="model-inputs" class="jd-inputs"></div>
<div id="annotation"><div id="annotation-label"></div><div id="annotation-title"></div><div id="annotation-body"></div></div>
<aside id="dashboard" class="jev-dashboard"></aside>
<p class="foot">同版引擎 · seed 42 · 输入摘要对应实际调用 · 完整字段见交互大盘</p>
</div><script>
const meta=__META__;const dashboard=document.getElementById('dashboard');dashboard.innerHTML=JevDashboard.markup();
const inputPanel=document.getElementById('model-inputs');inputPanel.innerHTML=JevDashboard.inputMarkup(false);
const driver={second:0};
function paint(second){
 let p=meta.parts[meta.parts.length-1];for(const part of meta.parts){if(second<part.start+part.duration){p=part;break;}}
 const c=meta.chapters[p.chapter];const elapsed=p.normal_start+(p.hold?0:Math.max(0,second-p.start));
 document.getElementById('chapter-label').textContent=`${String(p.chapter+1).padStart(2,'0')} / ${meta.chapters.length}　${c.label}`;
 let jevSecond=0;
 for(const name of ['script','jev']){
   const side=c.sides[name],held=elapsed>=side.duration;
   const source=held?side.still_source_s:Math.min(side.source_end-1/60,side.source_start+elapsed*c.rate);
   document.getElementById(name+'-status').textContent=`原片 ${JevDashboard.clock(source)}　${held?'章末清晰帧 · 定格等候':p.hold?'关键节点 · 停驻解说':'按原顺序回放'}`;
   if(name==='jev')jevSecond=source;
 }
 JevDashboard.render(dashboard,jevSecond);
 JevDashboard.renderInputs(inputPanel,jevSecond);
 const note=p.note||JevDashboard.noteAt(jevSecond);
 document.getElementById('rate').textContent=p.hold?'关键节点停驻 8 秒 · 两侧画面同时暂停':`${c.rate}× 模拟帧回放 · 本章先完成的一侧定格等候`;
 document.getElementById('annotation-label').textContent=note?note.label:'观看提示';
 document.getElementById('annotation-title').textContent=note?note.title:'对照目标、动作与实际结果';
 document.getElementById('annotation-body').textContent=note?note.body:meta.guides[p.chapter];
}
const tl=gsap.timeline({paused:true});tl.to(driver,{second:__DURATION__,duration:__DURATION__,ease:'none',onUpdate:()=>paint(driver.second)},0);paint(0);window.__timelines.main=tl;
</script></body></html>'''
    (project/'index.html').write_text(page.replace('__DURATION__',str(cursor)).replace('__CONTENT__','\n'.join(content)).replace('__META__',json.dumps(meta,ensure_ascii=False).replace('</','<\\/')))
    return cursor,parts
