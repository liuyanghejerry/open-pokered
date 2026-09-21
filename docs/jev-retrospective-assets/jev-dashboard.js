/* Recorded observations and typed choices only. Seek-safe; no media control. */
(function () {
  const t = value => window.JevI18n ? JevI18n.t(value) : String(value);
  const localizedHtml = value => window.JevI18n ? JevI18n.html(value) : value;
  const esc = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const at = (rows, second) => { let lo=0, hi=rows.length; while(lo<hi){const mid=(lo+hi)>>1;if(rows[mid][0]<=second+0.00001)lo=mid+1;else hi=mid;}return lo?rows[lo-1][1]:null; };
  const clock = value => {const s=Math.max(0,Math.floor(value));return `${Math.floor(s/3600)}:${String(Math.floor(s%3600/60)).padStart(2,'0')}:${String(s%60).padStart(2,'0')}`;};
  function markup(){return localizedHtml(`<div class="jd-head"><span class="jd-eyebrow">JEV 决策与执行记录</span><span class="jd-badges"></span></div>
    <div class="jd-location"></div><div class="jd-current-label">游戏当前状态 · 区别于下方决策输入快照</div><div class="jd-party"></div>
    <section class="jd-strategy"><div class="jd-kicker">策略层 · 最近一次选择</div><div class="jd-goal"></div><div class="jd-prob-label">候选前三项 · 选择分布，不是成功率</div><div class="jd-candidates"></div></section>
    <section class="jd-action-section"><div class="jd-kicker">动作层 / 通用技能执行</div><div class="jd-action"></div></section>
    <section class="jd-command-section"><div class="jd-kicker">执行器最近输入 · 按观测帧对齐</div><div class="jd-command"></div><div class="jd-command-age"></div></section>
    <section class="jd-result-section"><div class="jd-kicker">已观察到的执行结果</div><div class="jd-result"></div><div class="jd-result-detail"></div></section>
    <div class="jd-counts"></div><div class="jd-clock"></div>`);}
  function render(root,second){
    const d=window.JEV_DASHBOARD;if(!root||!d)return;
    const state=at(d.states,second)||{},strategy=at(d.strategies,second)||{},action=at(d.actions,second)||{},result=at(d.outcomes,second)||{},counts=at(d.counts,second)||{};
    const put=(selector,text)=>{text=t(text);const e=root.querySelector(selector);if(e&&e.textContent!==text)e.textContent=text;};
    put('.jd-badges',`${state.badges||0}/8 徽章`);
    const special={title:'标题画面',main_menu:'主菜单',copyright:'版权画面'};
    put('.jd-location',state.hof?'名人堂已记录 · '+(state.process===2?'独立读档验证':state.place||'结局流程'):(special[state.screen]||state.place||'从 NEW GAME 开始'));
    const party=(state.party||[]).map(m=>`<div class="jd-mon"><div class="jd-mon-label"><b>${esc(t(m.name))} <span>Lv.${m.level}</span></b><span>${m.hp}/${m.max_hp} HP</span></div><div class="jd-hp"><i style="width:${Math.max(0,Math.min(100,m.hp/m.max_hp*100))}%;background:${m.hp/m.max_hp<.25?'#a3472b':'#237451'}"></i></div></div>`).join('')||'<div class="jd-empty">尚未领取初始宝可梦</div>';
    const partyRoot=root.querySelector('.jd-party');if(partyRoot.innerHTML!==localizedHtml(party))partyRoot.innerHTML=localizedHtml(party);
    put('.jd-goal',strategy.title||'等待首次策略选择');
    put('.jd-prob-label',strategy.model_choice?'分布非成功率 · 本次由代码从非拒选项中选择':'候选前三项 · 选择分布，不是成功率');
    const candidates=(strategy.candidates||[]).map(c=>`<div class="jd-option ${c.selected?'jd-selected':''}"><div class="jd-option-label"><span>${c.selected?'✓ ':''}${esc(t(c.label))}</span><b>${Math.round(c.p*100)}%</b></div><div class="jd-prob"><i style="width:${c.p*100}%"></i></div></div>`).join('')||'<div class="jd-empty">尚无模型判断</div>';
    const candidateRoot=root.querySelector('.jd-candidates');if(candidateRoot.innerHTML!==localizedHtml(candidates))candidateRoot.innerHTML=localizedHtml(candidates);
    put('.jd-action',action.title||'开机与新建游戏');
    put('.jd-action-section .jd-kicker',({'recorded judgment':'动作层 · Jev 选择，由技能执行','cached judgment':'动作层 · 复用已缓存的 Jev 选择','code fallback':'通用技能 · 代码选择合法回合'})[action.origin]||'动作层 / 通用技能执行');
    const command=at(d.commands||[],second);
    put('.jd-command',command?.label||'尚无已完成输入');
    put('.jd-command-age',command?`原片 ${clock(command.source_s)} · ${command.ok?'命令成功':'命令失败'} · 不代表持续按住`:'通用技能负责按键、导航与推进对白');
    put('.jd-result',state.hof?'冠军目标已完成':result.title||'等待首个操作结果');
    put('.jd-result-detail',state.process===2?'新进程载入自然结局存档':result.detail||'以游戏状态变化确认进度');
    put('.jd-counts',`判断 ${counts.strategy||0} 策略 / ${counts.action||0} 动作　·　战败 ${counts.defeats||0}`);
    put('.jd-clock',`Jev 原片 ${clock(second)} · 状态与选择均来自记录`);
  }
  let rawLoading;
  function loadRaw(){
    if(window.JEV_INPUTS)return Promise.resolve(window.JEV_INPUTS);
    if(!rawLoading)rawLoading=new Promise((resolve,reject)=>{const tag=document.createElement('script');const base=document.querySelector('script[src$="jev-dashboard-data.js"]');tag.src=new URL('jev-inputs-data.js',base.src).href;tag.onload=()=>resolve(window.JEV_INPUTS);tag.onerror=()=>{rawLoading=null;tag.remove();reject(new Error(t('输入文件加载失败，请保留 jev-inputs-data.js。')));};document.head.appendChild(tag);});
    return rawLoading;
  }
  function inputMarkup(interactive=true){return localizedHtml(['strategy','action'].map(layer=>`<section class="jd-input-card" data-layer="${layer}"><div class="jd-input-title"></div><div class="jd-input-meta"></div><dl class="jd-input-rows"></dl>${interactive?'<details class="jd-raw"><summary>展开当次完整输入、全部候选与输出</summary><p>应用层调用原文：state 是状态，question 是指令与全部候选，answer 是模型输出；controller_selection 如存在，表示代码二次选择。此处不包含模型隐藏推理。</p><pre></pre></details>':''}</section>`).join(''));}
  async function showRaw(card){const detail=card.querySelector('.jd-raw');if(!detail?.open)return;const id=card.dataset.inputId;const pre=detail.querySelector('pre');if(!id){pre.textContent=t('该层尚未发生模型调用。');return;}try{const all=await loadRaw();if(card.dataset.inputId===id)pre.textContent=JSON.stringify(all[id],null,2);}catch(error){pre.textContent=error.message;}}
  function renderInputs(root,second){if(!root||!window.JEV_DASHBOARD)return;for(const card of root.querySelectorAll('.jd-input-card')){
    const layer=card.dataset.layer,entry=at(window.JEV_DASHBOARD[layer+'_inputs']||[],second),id=String(entry?.id||'');
    if(card.dataset.inputId!==id){card.dataset.inputId=id;
      card.querySelector('.jd-input-title').textContent=t(`${layer==='strategy'?'策略层':'动作层'} · 当次输入${entry?' #'+entry.id+' / '+entry.kind:''}`);
      card.querySelector('.jd-input-rows').innerHTML=entry?entry.rows.map(([k,v])=>`<dt>${esc(t(k))}</dt><dd title="${esc(t(v))}">${esc(t(v))}</dd>`).join(''):localizedHtml('<dd>该层尚未发生模型调用</dd>');
      showRaw(card);
    }
    card.querySelector('.jd-input-meta').textContent=t(entry?`原片快照 ${clock(entry.source_s)} · 距画面 ${Math.max(0,Math.floor(second-entry.source_s))} 秒 · ${entry.non_abstain_candidates} 个候选`:'只展示实际传入信息，不以当前状态补齐');
    const detail=card.querySelector('.jd-raw');if(detail&&!detail.dataset.bound){detail.dataset.bound='1';detail.addEventListener('toggle',()=>showRaw(card));}
  }}
  function noteAt(second){const notes=window.JEV_DASHBOARD?.notes||[];const note=[...notes].reverse().find(n=>n.source_s<=second+0.00001&&second<n.source_s+240);return note?{...note,title:t(note.title),body:t(note.body),label:t(note.label)}:null;}
  window.JevDashboard={markup,render,at,clock,noteAt,inputMarkup,renderInputs};
})();
