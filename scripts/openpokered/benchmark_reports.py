"""Multi-model reports. Preserve missing evidence and early-stop reasons."""
import collections
import csv
import json
import math
import statistics
from pathlib import Path

from .evaluation import atomic_json


def rows(path, allow_truncated=False):
    lines = path.read_bytes().splitlines(keepends=True) if path.exists() else []
    result = []
    for i, line in enumerate(lines):
        if not line.strip():
            continue
        try:
            result.append(json.loads(line))
        except (ValueError, UnicodeError):
            if allow_truncated and i == len(lines)-1 and not line.endswith(b'\n'):
                break  # Only a killed worker's unfinished final write is salvageable.
            raise
    return result


def outcome(summary, requests):
    reason = summary.get('reason', 'missing_result')
    errors = ' '.join(r.get('error', '') for r in requests)
    if 'HTTP 402' in errors:
        return 'service_credits_exhausted'
    if 'no_selection' in reason or 'no_candidates' in reason:
        return 'model_abstention'
    if reason == 'completed':
        return 'completed'
    if reason in ('effective_time_budget', 'absolute_wall_budget', 'supervisor_deadline'):
        return 'time_limit'
    if 'protocol_error' in errors or 'protocol_error' in reason:
        return 'model_protocol_error'
    if 'service_unavailable' in reason or reason == 'model_error':
        return 'model_service_error'
    if reason in ('worker_failed', 'watchdog_termination'):
        return 'watchdog_termination'
    if not summary.get('clock', {}).get('start_monotonic'):
        return 'setup_error'
    return 'runtime_error'


def mean(values):
    values = [v for v in values if v is not None]
    return statistics.mean(values) if values else None


def decision_metrics(summary):
    trials = summary.get('trials', [])
    valid = [t for t in trials if t.get('valid')]
    by_case = collections.defaultdict(list)
    groups = collections.defaultdict(list)
    for trial in valid:
        by_case[trial['case_id']].append(trial)
        groups[trial['layer']].append(trial)
        groups[trial['category']].append(trial)
    pairs = [ts for ts in by_case.values() if {t['order'] for t in ts} == {'original', 'reversed'}]
    expected = summary.get('expected_trials')
    return {'valid_trials': len(valid), 'expected_trials': expected, 'case_count': len(by_case),
            'coverage': len(valid)/expected if expected else None,
            'accuracy': mean([t['correct'] for t in valid]),
            'order_consistency': mean([len({t['choice'] for t in ts}) == 1 for ts in pairs]),
            'order_pair_count': len(pairs), 'brier': mean([t['brier'] for t in valid]),
            'negative_log_likelihood': mean([t['negative_log_likelihood'] for t in valid]),
            'probability_metric_trials': sum(t['negative_log_likelihood'] is not None for t in valid),
            'development_trials': sum(t['split'] == 'development' for t in valid),
            'groups': {k: {'valid_trials': len(ts), 'accuracy': mean([t['correct'] for t in ts])} for k, ts in groups.items()}}


def validate_run(run, plan, job):
    from .benchmark import digest
    summary, requests = run['summary'], run['requests']
    meta = summary.get('benchmark', {})
    if meta.get('comparison_key') != plan['comparison_key']:
        raise ValueError(f'Comparison fingerprint mismatch in {job["id"]}')
    if meta.get('conditions') != plan['conditions']:
        raise ValueError('Run conditions differ from frozen plan')
    models = {m['id']: m for m in plan['config']['models']}
    if meta.get('model_config_sha256') != digest({'config': models[job['model_id']],
                                                 'artifacts': plan['model_artifacts'][job['model_id']]}):
        raise ValueError('Model identity/configuration mismatch')
    for key in ('model_id', 'seed', 'repetition', 'track'):
        if meta.get(key) != job[key]:
            raise ValueError(f'Run metadata mismatch: {key}')
    model = summary.get('model')
    # A watchdog checkpoint may precede the final journal write. Keep it as
    # incomplete evidence rather than pretending exact accounting survived.
    incomplete = summary.get('reason') in ('worker_failed', 'watchdog_termination')
    if model and not incomplete:
        timed = [r for r in requests if not r['warmup']]
        if model['calls'] != len(timed):
            raise ValueError('Call count does not match request journal')
        for key in ('input_tokens', 'output_tokens'):
            if model[key] != sum(r.get(key) or 0 for r in timed):
                raise ValueError(f'{key} does not match request journal')
        if abs(summary['clock']['rtt_credit_s']-sum(r.get('rtt_credit_s', 0) for r in timed)) > 1e-6:
            raise ValueError('RTT total does not match request journal')
    if job['track'] == 'story':
        for key in ('source_sha256', 'binary_sha256'):
            if key in summary and summary[key] != plan['environment'][key]:
                raise ValueError(f'Runtime {key} differs from frozen plan')
        observations = run['observations']
        limit = plan['config']['story']['seconds']
        if any(r['effective_s'] > limit for r in observations):
            raise ValueError('Post-deadline gameplay observations cannot be scored')
        if any(a['effective_s'] > b['effective_s'] for a, b in zip(observations, observations[1:])):
            raise ValueError('Observations are not chronological')
        if observations and not incomplete and summary['metrics']['final'] != observations[-1]:
            raise ValueError('Final scored state does not match observation journal')


def flatten(run):
    summary, job = run['summary'], run['job']
    model = summary.get('model') or {}
    clock = summary.get('clock') or {}
    result = {**{k: job[k] for k in ('model_id', 'track', 'seed', 'repetition')},
              'outcome': outcome(summary, run['requests']), 'reason': summary.get('reason'),
              'raw_s': clock.get('raw_s'), 'effective_s': clock.get('effective_s'),
              'budget_s': clock.get('limit_s'),
              'shutdown_overrun_s': max(0, clock.get('effective_s', 0)-clock.get('limit_s', 0)),
              'rtt_credit_s': clock.get('rtt_credit_s'), 'calls': model.get('calls'),
              'input_tokens_known': model.get('input_tokens'), 'output_tokens_known': model.get('output_tokens'),
              'usage_missing_calls': model.get('usage_missing_calls'), 'latency_median_s': model.get('latency_median_s'),
              'latency_p95_s': model.get('latency_p95_s'),
              'encoding_reported_questions': (model.get('encoding') or {}).get('questions'),
              'setup_s': summary.get('setup_s'), 'actual_models': model.get('actual_models', [])}
    incomplete = summary.get('reason') in ('worker_failed', 'watchdog_termination')
    result['incomplete_evidence'] = incomplete
    if incomplete:
        result.update(raw_s=None, effective_s=None, shutdown_overrun_s=None,
                      warning='Last checkpoint only; final clock and pending request usage may be unavailable.')
    if job['track'] == 'decisions':
        result.update(decision_metrics(summary))
        return result
    metrics = summary.get('metrics') or {}
    final = metrics.get('final') or {}
    failures = metrics.get('failures') or {}
    money = metrics.get('money') or {}
    levels = metrics.get('party_levels') or {}
    observed = bool(final)
    def measured(value):
        return value if observed else None
    result.update(scored_observation=observed, story_objectives=measured(len(metrics.get('milestones', {}))),
                  scored_until_s=final.get('effective_s'),
                  objective_total=metrics.get('objective_total'), badges=final.get('badges'),
                  dex_seen=(final.get('pokedex') or {}).get('seen'), dex_owned=(final.get('pokedex') or {}).get('owned'),
                  sidequests=measured(len(metrics.get('sidequests', {}))), map_count=measured(len(metrics.get('maps', []))),
                  max_level=levels.get('max') if observed else None, mean_level=levels.get('mean') if observed else None,
                  total_level=levels.get('sum') if observed else None,
                  party_experience=measured(sum(m.get('total_exp', 0) for m in final.get('party', []))),
                  money_initial=money.get('initial'), money_final=money.get('final'), money_peak=money.get('peak'),
                  money_gained=money.get('observed_positive_deltas'), money_lost=money.get('observed_negative_deltas'),
                  battle_defeats=measured(failures.get('reported_battle_defeats', 0)),
                  observed_party_wipes=measured(failures.get('observed_party_wipes', 0)),
                  unsuccessful_operations=measured(failures.get('unsuccessful_operations', 0)),
                  rejected_actions=measured(failures.get('rejected_actions', 0)),
                  protocol_errors=measured(failures.get('protocol_errors', 0)),
                  suspected_stalls=measured(len(metrics.get('suspected_stalls', []))),
                  actions=summary.get('actions'), milestones=metrics.get('milestones', {}))
    return result


def fmt(value):
    if value is None:
        return '—'
    if isinstance(value, float):
        return f'{value:.3f}'
    return str(value).replace('|', '\\|').replace('\n', ' ')


def table(headers, values):
    return ['| ' + ' | '.join(headers) + ' |', '| ' + ' | '.join('---' for _ in headers) + ' |',
            *['| ' + ' | '.join(fmt(v) for v in row) + ' |' for row in values]]


def plot(runs, out):
    try:
        import matplotlib
        matplotlib.use('Agg')
        import matplotlib.pyplot as plt
    except ImportError:
        return False
    stories = [r for r in runs if r['job']['track'] == 'story' and r['observations']]
    if not stories:
        return False
    fig, axes = plt.subplots(2, 2, figsize=(12, 8))
    names = sorted({r['job']['model_id'] for r in stories})
    colors = {name: plt.get_cmap('tab10')(i % 10) for i, name in enumerate(names)}
    for run in stories:
        job = run['job']
        observations = run['observations']
        x = [r['effective_s']/60 for r in observations]
        series = ([len(r['completed_objectives']) for r in observations], [r['badges'] for r in observations],
                  [r['pokedex'].get('owned', 0) for r in observations], [r['money'] for r in observations])
        for ax, y in zip(axes.flat, series):
            ax.step(x, y, where='post', color=colors[job['model_id']], alpha=.65,
                    label=f'{job["model_id"]} s{job["seed"]} r{job["repetition"]}')
            ax.scatter(x[-1:], y[-1:], color=colors[job['model_id']], s=15)
    limit = runs[0]['summary']['benchmark']['conditions']['story']['seconds']/60
    for ax, title in zip(axes.flat, ('Story objectives', 'Badges', 'Pokedex owned', 'Money balance')):
        ax.set_title(title)
        ax.set_xlabel('Effective minutes')
        ax.set_xlim(0, limit)
        ax.grid(alpha=.2)
    axes[0, 0].legend(fontsize=7)
    fig.suptitle('Observed progress; lines stop at the last scored sample')
    fig.tight_layout()
    fig.savefig(out/'progress.png', dpi=140)
    fig.savefig(out/'progress.svg')
    plt.close(fig)
    return True


def report(folders, out):
    from .benchmark import case_digest, digest, sha_file
    plans, runs, identities, actual_models, seen = [], [], {}, {}, set()
    for folder in folders:
        plan = json.loads((folder/'plan.json').read_text())
        if digest(plan['conditions']) != plan['comparison_key']:
            raise ValueError('Invalid frozen plan fingerprint')
        if plan.get('cases') and case_digest(plan['cases']) != plan['conditions']['cases_sha256']:
            raise ValueError('Frozen cases, labels or option order changed')
        if plans and plan['comparison_key'] != plans[0]['comparison_key']:
            raise ValueError('Non-comparable benchmark conditions; use separate reports for different versions, hardware, budgets or smoke runs')
        plans.append(plan)
        for model in plan['config']['models']:
            fingerprint = digest({'config': model, 'artifacts': plan['model_artifacts'][model['id']]})
            if model['id'] in identities and identities[model['id']] != fingerprint:
                raise ValueError('A model ID refers to different configurations; rename the variant')
            identities[model['id']] = fingerprint
        for job in plan['jobs']:
            identity = (job['model_id'], job['track'], job['seed'], job['repetition'])
            if identity in seen:
                raise ValueError('Duplicate model/track/seed/repetition; do not count an episode twice')
            seen.add(identity)
            path = folder/job['id']
            if not (path/'summary.json').exists():
                raise ValueError(f'Incomplete campaign: missing {job["id"]}/summary.json')
            summary = json.loads((path/'summary.json').read_text())
            incomplete = summary.get('reason') in ('worker_failed', 'watchdog_termination')
            run = {'path': path, 'job': job, 'summary': summary,
                   'requests': rows(path/'requests.jsonl', incomplete), 'observations': rows(path/'observations.jsonl', incomplete)}
            validate_run(run, plan, job)
            actual = tuple(sorted((run['summary'].get('model') or {}).get('actual_models', [])))
            if actual:
                if job['model_id'] in actual_models and actual_models[job['model_id']] != actual:
                    raise ValueError('Actual model version changed under the same model ID; register a separate variant')
                actual_models[job['model_id']] = actual
            runs.append(run)
    # A comparison may add new models in later campaigns, but each model must
    # have the same predeclared seed/repetition coverage in a given track.
    coverage = collections.defaultdict(lambda: collections.defaultdict(set))
    for run in runs:
        job = run['job']
        coverage[job['track']][job['model_id']].add((job['seed'], job['repetition']))
    for by_model in coverage.values():
        sets = list(by_model.values())
        if any(s != sets[0] for s in sets):
            raise ValueError('Unequal seed/repetition coverage between models')
    out.mkdir(parents=True, exist_ok=False)
    flat = [flatten(r) for r in runs]
    columns = sorted({key for row in flat for key in row})
    with (out/'metrics.csv').open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=columns)
        writer.writeheader()
        for row in flat:
            writer.writerow({k: json.dumps(v, ensure_ascii=False) if isinstance(v, (dict, list)) else v for k, v in row.items()})
    grouped = collections.defaultdict(list)
    for row in flat:
        grouped[(row['track'], row['model_id'])].append(row)
    aggregates = []
    for (track, model), group in grouped.items():
        numeric = ('accuracy', 'coverage', 'order_consistency', 'brier') if track == 'decisions' else ('story_objectives', 'badges', 'dex_owned', 'sidequests', 'max_level', 'money_gained', 'effective_s')
        aggregate = {'model_id': model, 'track': track, 'runs': len(group),
                     'outcomes': dict(collections.Counter(r['outcome'] for r in group))}
        for key in numeric:
            values = [r[key] for r in group if r.get(key) is not None]
            aggregate[key] = {'n': len(values), 'mean': mean(values), 'median': statistics.median(values) if values else None,
                              'min': min(values) if values else None, 'max': max(values) if values else None,
                              'stdev': statistics.stdev(values) if len(values) > 1 else None}
        aggregates.append(aggregate)
    audit = {'valid': True, 'comparison_key': plans[0]['comparison_key'], 'conditions': plans[0]['conditions'],
             'sources': [{ 'path': str(r['path']), 'sha256': {name: sha_file(r['path']/name)
                         for name in ('summary.json', 'requests.jsonl', 'observations.jsonl', 'trace.jsonl', 'commands.jsonl')
                         if (r['path']/name).exists()}} for r in runs]}
    atomic_json(out/'verification.json', audit)
    atomic_json(out/'results.json', {'runs': flat, 'aggregates': aggregates})
    text = ['# Open-Pokered 模型 Benchmark', '',
            f'版本 `{plans[0]["benchmark"]}` · 配置 `{plans[0]["conditions"]["profile"]}` · 用途 **{plans[0]["conditions"]["purpose"]}**。', '',
            '每行保留独立运行；未完成预算的结果不外推。服务中断、模型拒选、协议错误与超时分别列出。',
            '本报告不计算加权总分，也不把提前结束的任务排成完整预算能力榜。', '']
    for track, title, keys, labels in [
        ('decisions', '固定场景决策', ['accuracy', 'coverage', 'order_consistency', 'brier'], ['有效答案正确率', '覆盖率', '顺序一致率', 'Brier']),
        ('story', '限时自主探索', ['scored_until_s', 'story_objectives', 'badges', 'dex_seen', 'dex_owned', 'sidequests', 'max_level', 'battle_defeats', 'money_gained'],
         ['最后计分秒', '剧情目标', '徽章', '图鉴见过', '图鉴拥有', '支线', '最高等级', '战败', '观测收入']),
    ]:
        selected = [r for r in flat if r['track'] == track]
        if not selected:
            continue
        text += [f'## {title}', '']
        text += table(['模型', '种子/重复', '结束类别', *labels],
                      [[r['model_id'], f'{r["seed"]}/{r["repetition"]}', r['outcome'], *[r.get(k) for k in keys]] for r in selected])
        text += ['', '### 跨运行汇总', '']
        for aggregate in [a for a in aggregates if a['track'] == track]:
            text += [f'**{aggregate["model_id"]}**：{aggregate["runs"]} 次；结束类别 `{json.dumps(aggregate["outcomes"], ensure_ascii=False)}`。', '']
            text += table(['指标', '有效样本数', '均值', '中位数', '最小', '最大', '标准差'],
                          [[k, *[v[f] for f in ('n', 'mean', 'median', 'min', 'max', 'stdev')]]
                           for k, v in aggregate.items() if isinstance(v, dict) and 'median' in v])
            text.append('')
    text += ['## 成本与调用', '']
    text += table(['模型', '轨道', '种子/重复', '调用数', '已知输入 Token', '已知输出 Token', '用量缺失调用', 'RTT 扣减秒', 'P50 秒', 'P95 秒'],
                  [[r['model_id'], r['track'], f'{r["seed"]}/{r["repetition"]}', *[r.get(k) for k in ('calls', 'input_tokens_known', 'output_tokens_known', 'usage_missing_calls', 'rtt_credit_s', 'latency_median_s', 'latency_p95_s')]] for r in flat])
    text += ['', '### 时间边界', '']
    text += table(['模型', '轨道', '种子/重复', '预算秒', '原始墙钟秒', '扣 RTT 后墙钟秒', '超预算收尾秒'],
                  [[r['model_id'], r['track'], f'{r["seed"]}/{r["repetition"]}',
                    *[r.get(k) for k in ('budget_s', 'raw_s', 'effective_s', 'shutdown_overrun_s')]] for r in flat])
    text += ['', 'Token 缺失时已知合计只是下界；不同 tokenizer 的数量不能直接等价比较计算成本。金额是采样余额正变化的下界。',
             '墙钟可能包含等待在途操作结束的收尾时间；收尾不增加剧情成绩，最后计分观测始终在预算内。',
             'watchdog/worker_failed 只保留最后 checkpoint；未知最终墙钟不显示为 0，未结束请求的用量也可能缺失。',
             '决策正确率只统计有效答案，必须同时看覆盖率；概率指标仅统计返回完整分布的答案，不为仅返回标签的模型伪造概率。',
             '当前固定场景集包含已公开调试样本，属于 development 集。选项反转和重复运行不增加独立场景数量，也不能称为隐藏测试集准确率。', '']
    stories = [r for r in flat if r['track'] == 'story']
    if stories:
        text += ['## 剧情节点时间', '']
        milestones = sorted({key for r in stories for key in r['milestones']})
        text += table(['模型', '种子/重复', '剧情节点', '有效秒', '原始秒'],
                      [[r['model_id'], f'{r["seed"]}/{r["repetition"]}', key,
                        r['milestones'].get(key, {}).get('effective_s'), r['milestones'].get(key, {}).get('raw_s')]
                       for r in stories for key in milestones])
        text += ['', '未达到节点为空值，不能按结束时间当作完成时间。支线计分使用预定义观察目录，当前控制器以通关为目标。', '']
    if plot(runs, out):
        text += ['![实际剧情推进曲线](progress.png)', '']
    text += ['[全部指标 CSV](metrics.csv) · [逐次结果与汇总 JSON](results.json) · [来源哈希与核验](verification.json)', '']
    (out/'README.md').write_text('\n'.join(text))
    return flat
