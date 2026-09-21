#!/usr/bin/env python3
"""Build an auditable comparison from two finished, fixed-budget runs."""
import argparse
import collections
import csv
import hashlib
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0,str(Path(__file__).resolve().parent.parent))

def read_rows(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def load_run(path):
    return {'folder':path,'summary':json.loads((path/'summary.json').read_text()),
            'states':read_rows(path/'observations.jsonl'),
            'requests':[r for r in read_rows(path/'requests.jsonl') if not r['warmup']],
            'trace':read_rows(path/'trace.jsonl')}


def validate(runs):
    a,b=[r['summary'] for r in runs]
    for key in ('seed','profile','target','clock_mode','seconds','binary_sha256','source_sha256','python'):
        if a[key]!=b[key]:raise ValueError(f'Non-comparable {key}: {a[key]} != {b[key]}')
    if [a['backend'],b['backend']]!=['jev','laya']:raise ValueError('Require Jev then Laya in the corresponding CLI arguments')
    if a['seconds']!=1200:raise ValueError('The formal report requires a 20-minute budget')
    if a['clock']['start_monotonic']+a['clock']['raw_s']>b['clock']['start_monotonic']:
        raise ValueError('Timed runs overlap or were not executed in Jev then Laya order on this host')
    for run in runs:
        s=run['summary'];model=s['model'];states=run['states'];requests=run['requests']
        assert all(x['effective_s']<=s['seconds'] for x in states)
        assert all(x['effective_s']<=y['effective_s'] for x,y in zip(states,states[1:]))
        assert model['calls']==len(requests)
        assert model['input_tokens']==sum(r.get('input_tokens',0) for r in requests)
        assert model['output_tokens']==sum(r.get('output_tokens',0) for r in requests)
        assert abs(s['clock']['rtt_credit_s']-sum(r['rtt_credit_s'] for r in requests))<1e-6
        if s['backend']=='laya':assert s['clock']['rtt_credit_s']==0
        if s['metrics']['final']!=states[-1]:raise ValueError('Scored final snapshot mismatch')


def format_value(value):
    if value is None:return '—'
    if isinstance(value,float):return f'{value:,.3f}'
    if isinstance(value,int):return f'{value:,}'
    return str(value)


def metric_rows(runs):
    def get(run):
        s=run['summary'];m=s['metrics'];f=m['final'];model=s['model'];clock=s['clock']
        layers=collections.Counter(q for r in run['requests'] for q in r['question_ids'])
        return {
            '退出原因':s['reason'],'原始墙钟（秒）':clock['raw_s'],'RTT 扣减（秒）':clock['rtt_credit_s'],
            '有效墙钟（秒）':clock['effective_s'],'最后计分观测（秒）':f['effective_s'],
            '截止后等待 / 退出耗时（秒）':s.get('deadline_overrun_s',0),
            '完成主线目标 / 11':len(m['milestones']),'徽章 / 8':f['badges'],
            '最终地图':f['map'],'已见图鉴 / 151':f['pokedex']['seen'],'已拥有图鉴 / 151':f['pokedex']['owned'],
            '可选成就 / 11':len(m['sidequests']),'采样观察地图数':len(m['maps']),
            '判断调用数':model['calls'],'策略 / 动作调用':f"{layers['strategy']} / {layers['action']}",
            '失败模型请求数':model['calls']-model['successful_calls'],
            '未返回 Token 用量的请求数':model['usage_missing_calls'],
            '截止后返回的模型请求数':sum(r['clock']['effective_s']>s['seconds'] for r in run['requests']),
            '输入 Token':model['input_tokens'],'输出 Token':model['output_tokens'],
            '累计请求耗时（秒）':model['latency_sum_s'],'请求时延中位数（秒）':model['latency_median_s'],
            '请求时延 P95（秒）':model['latency_p95_s'],'可测 RTT 样本数':model['rtt_measurements'],
            '语义操作数':s.get('actions',0),'选招缓存命中数':s.get('action_cache_hits',0),
            '模拟帧':f['frame'],'队伍最高 / 平均 / 总等级':f"{m['party_levels']['max']} / {m['party_levels']['mean']:.2f} / {m['party_levels']['sum']}",
            '初始余额':m['money']['initial'],'最终余额':m['money']['final'],'峰值余额':m['money']['peak'],
            '观测正余额变化累计':m['money']['observed_positive_deltas'],
            '观测负余额变化累计':m['money']['observed_negative_deltas'],
            '代理报告战败':m['failures'].get('reported_battle_defeats',0),
            '观察全队 HP 归零':m['failures'].get('observed_party_wipes',0),
            '无效 / 受阻操作':m['failures'].get('unsuccessful_operations',0),
            '其中旅行受阻':m['failures'].get('blocked_travel_operations',0),
            '被拒动作':m['failures'].get('rejected_actions',0),'协议错误':m['failures'].get('protocol_errors',0),
            '疑似停滞区间数':len(m['suspected_stalls']),'计时前初始化（秒）':s['setup_s'],
            '预热输入 Token':model['warmup']['input_tokens'],
        }
    values=[get(run) for run in runs]
    return [[key,*[v[key] for v in values]] for key in values[0]]


def figures(runs,out):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    from matplotlib import font_manager
    font='/Library/Fonts/Arial Unicode.ttf'
    font_manager.fontManager.addfont(font)
    plt.rcParams.update({'font.family':font_manager.FontProperties(fname=font).get_name(),
        'axes.unicode_minus':False,'svg.fonttype':'path','font.size':11,
        'figure.facecolor':'#f6f4ee','axes.facecolor':'#f6f4ee',
        'axes.spines.top':False,'axes.spines.right':False})
    colors={'jev':'#237451','laya':'#536d8d'}
    fig,axes=plt.subplots(2,2,figsize=(12,8))
    for run in runs:
        s=run['summary'];name=s['backend'];states=run['states'];x=[r['effective_s']/60 for r in states]
        for ax,values in zip(axes.flat,(
            [len(r['completed_objectives']) for r in states],
            [r['badges'] for r in states],
            [max((m['level'] for m in r['party']),default=0) for r in states],
            [r['money'] for r in states])):
            ax.step(x,values,where='post',color=colors[name],label=name,linewidth=2)
            if x:ax.scatter([x[-1]],[values[-1]],color=colors[name],s=35,zorder=3)
            ax.set_xlim(0,20);ax.set_xlabel('有效时间（分钟）');ax.grid(alpha=.15)
        if x and x[-1]<19.9:
            axes[0,0].annotate(f'{name} 提前停止：{x[-1]*60:.1f} 秒',
                (x[-1],len(states[-1]['completed_objectives'])),xytext=(20,30),textcoords='offset points',
                arrowprops={'arrowstyle':'->','color':colors[name]},color=colors[name])
    for ax,title in zip(axes.flat,('已完成主线目标（共 11 项）','已获徽章（共 8 枚）','队伍最高等级','当前金钱余额')):
        ax.set_title(title,loc='left');ax.legend(frameon=False)
    fig.suptitle('20 分钟预算内的实际推进',x=.07,ha='left',fontsize=21)
    fig.text(.07,.018,'线条止于最后计分观测；提前停止不补造 20 分钟的状态。图鉴、支线与成本详见指标表。',fontsize=10)
    fig.tight_layout(rect=(0,.035,1,.94))
    for ext in ('png','svg'):fig.savefig(out/f'progress.{ext}',dpi=160)
    plt.close(fig)
    fig,axes=plt.subplots(1,2,figsize=(10,5.2))
    for ax,run in zip(axes,runs):
        s=run['summary'];path=run['folder']/'final.png'
        if path.exists():ax.imshow(plt.imread(path),interpolation='nearest')
        ax.axis('off');ax.set_title(f"{s['backend']} · {s['metrics']['final']['map']}\n{s['reason']}",fontsize=12)
    fig.suptitle('退出后的真实画面（诊断截图，成绩采用预算内采样）',fontsize=16)
    fig.tight_layout(rect=(0,0,1,.9));fig.savefig(out/'final-screens.png',dpi=160);plt.close(fig)


def report(runs,out):
    out.mkdir(parents=True,exist_ok=True);validate(runs)
    rows=metric_rows(runs)
    with (out/'metrics.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['指标','Jev','Laya']);w.writerows(rows)
    for run in runs:
        name=run['summary']['backend']
        shutil.copy2(run['folder']/'summary.json',out/f'{name}-summary.json')
        shutil.copy2(run['folder']/'supervisor.json',out/f'{name}-supervisor.json')
    cases={}
    for run in runs:
        name=run['summary']['backend']
        cases[name]={'first_requests':run['requests'][:6],
            'opening_events':[r for r in run['trace'] if r['kind'] in ('conditional_choice','strategy','outcome','action_rejected')][:8],
            'ending_events':[r for r in run['trace'] if r['kind'] in ('stopped','action_rejected','battle_defeat','judgment_error')][-8:]}
    jev_by_input={r['request_sha256']:r for r in runs[0]['requests']}
    matches=[(jev_by_input[r['request_sha256']],r) for r in runs[1]['requests'] if r['request_sha256'] in jev_by_input]
    cases['identical_application_inputs']=[{'sha256':j['request_sha256'],'question_ids':j['question_ids'],
        'jev':{'number':j['number'],'answers':j.get('answers'),'latency_s':j['latency_s']},
        'laya':{'number':l['number'],'answers':l.get('answers'),'latency_s':l['latency_s']}}
        for j,l in matches]
    (out/'decision-evidence.json').write_text(json.dumps(cases,ensure_ascii=False,indent=2)+'\n')
    audit={'valid':True,'checks':['same seed / controller hash / game binary / Python / budget',
        'Jev timed run ends before Laya timed run starts on the same host',
        'all scored observations within budget and chronological','token and RTT totals match request journals',
        'final scored snapshot matches observation journal'],'sources':[]}
    for run in runs:
        audit['sources'].append({'folder':str(run['folder'].resolve()),'files':{
            name:hashlib.sha256((run['folder']/name).read_bytes()).hexdigest()
            for name in ('summary.json','requests.jsonl','observations.jsonl','trace.jsonl','commands.jsonl')}})
    (out/'verification.json').write_text(json.dumps(audit,ensure_ascii=False,indent=2)+'\n')
    figures(runs,out)
    text=['# Laya MLX 与 Jev：20 分钟预算下的自主探索实测','',
          '本页由两轮原始记录生成。实验条件与指标定义见[评测框架](../laya-jev-evaluation-framework.md)。',
          '同一控制器、二进制、种子和上限；初始化单列，Jev 仅扣实测 RTT。一次运行用于案例分析，不代表平均表现或成功率。','']
    if all(run['summary']['clock']['effective_s']<1200 for run in runs):
        text+=['**两轮均提前结束，未取得完整 20 分钟终点成绩。** 曲线不延长到预算终点，剩余时间也不外推为进度。','']
    if any('HTTP 402' in r.get('error','') and 'billing_error' in r.get('error','') for r in runs[0]['requests']):
        text+=['Jev 因 HTTP 402 额度不足停止。这是外部服务限制，不能归为模型无法继续推进。该失败请求没有返回 usage，表中 Token 是已报告部分。','']
    if (out/'interpretation.md').exists():
        text+=['原生后端适配、提前结束原因和后续实验建议见[本轮分析](interpretation.md)。','']
    text+=['| 指标 | Jev | Laya |','|---|---:|---:|']
    text.extend('| '+' | '.join(format_value(v) for v in row)+' |' for row in rows)
    text+=['','输入 Token 来自不同 tokenizer，不能按数量直接等价比较算力。Laya 无文本生成，输出 0 不代表零推断开销。金额变化是采样可见下界；失败各项可能重叠。','',
           '![剧情、徽章、队伍练度与余额](progress.png)','',
           '## 各主线目标首次观测完成时间','',
           '| 目标 | Jev 有效 / 原始秒 | Laya 有效 / 原始秒 |','|---|---:|---:|']
    from openpokered.judgment_agent import load_objectives
    for objective in load_objectives():
        cells=[]
        for run in runs:
            milestone=run['summary']['metrics']['milestones'].get(objective['id'])
            absent='提前结束前未达到' if run['summary']['clock']['effective_s']<1200 else '预算内未达到'
            cells.append(f"{milestone['effective_s']:.3f} / {milestone['raw_s']:.3f}" if milestone else absent)
        text.append('| '+objective['id']+' | '+' | '.join(cells)+' |')
    text+=['','## 队伍、图鉴与支线','']
    for run in runs:
        s=run['summary'];m=s['metrics'];f=m['final']
        text.append(f"**{s['backend']}**：拥有图鉴编号 {f['pokedex']['owned_numbers']}；可选成就 {list(m['sidequests']) or '无'}。")
        text+=['','| 成员 | 等级 | 经验 | HP | 招式 / PP |','|---|---:|---:|---:|---|']
        for mon in f['party']:
            moves=', '.join(f'{move} {pp}' for move,pp in zip(mon['moves'],mon['pp']) if move!='None')
            text.append(f"| {mon['species']} | {mon['level']} | {mon.get('total_exp','—')} | {mon['hp']}/{mon['max_hp']} | {moves} |")
        if not f['party']:text.append('| 尚未获得精灵 | — | — | — | — |')
        text.append('')
    laya=next(r['summary'] for r in runs if r['summary']['backend']=='laya')['model']['encoding']
    text+=['## 原生输入截断','',
        f"Laya 的 {laya['questions']} 道判断中，{laya['questions_with_state_truncation']} 道发生状态截断；状态共删去 {laya['state_tokens_dropped']:,} Token，候选共删去 {laya['candidate_tokens_dropped']:,} Token，问题说明共删去 {laya['instruction_tokens_dropped']:,} Token。",
        '实际编码文本、完整应用输入、原始输出以及开始/结束阶段事件见 [decision-evidence.json](decision-evidence.json)。截断与结果同时出现只能作为待验证解释，不能替代受控消融。','',
        '## 完全相同应用输入上的判断','',
        '以下依据完整请求 SHA-256 匹配，表内是原始模型输出；条件选择、拒绝处理等控制器行为另见事件记录。','',
        '| 问题 | Jev 原始选择 | Laya 原始选择 | Jev / Laya 请求秒 |','|---|---|---|---:|']
    for j,l in matches[:10]:
        selections=[]
        for request in (j,l):
            selections.append('; '.join(f"{key}: {answer.get('choice')}" for key,answer in request.get('answers',{}).items()))
        text.append(f"| {', '.join(j['question_ids'])} | {selections[0]} | {selections[1]} | {j['latency_s']:.3f} / {l['latency_s']:.3f} |")
    if not matches:text.append('| 本轮没有完全相同的请求 | — | — | — |')
    text+=['',
        '![两轮退出后的实际截图](final-screens.png)','',
        '## 证据与复现','',
        '[Jev 摘要](jev-summary.json) · [Laya 摘要](laya-summary.json) · [CSV](metrics.csv) · [原始记录哈希与核验](verification.json)。原始请求、观察和操作日志的本机路径保存在核验文件中。','']
    (out/'README.md').write_text('\n'.join(text))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jev',type=Path,required=True);parser.add_argument('--laya',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();report([load_run(args.jev),load_run(args.laya)],args.output)


if __name__=='__main__':main()
