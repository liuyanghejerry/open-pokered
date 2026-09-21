#!/usr/bin/env python3
"""Extract actual ending frames and a labeled proof sheet from full recordings."""
import argparse
import json
import subprocess
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib import font_manager


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--analysis', type=Path, required=True)
    ap.add_argument('--media', type=Path, required=True)
    ap.add_argument('--font', default='/Library/Fonts/Arial Unicode.ttf')
    args = ap.parse_args()
    records = json.loads(args.analysis.read_text())
    manifest = json.loads((args.media / 'manifest.json').read_text())
    font_manager.fontManager.addfont(args.font)
    plt.rcParams['font.family'] = font_manager.FontProperties(fname=args.font).get_name()
    plt.rcParams['svg.fonttype'] = 'path'
    fig, axes = plt.subplots(3, 2, figsize=(10.5, 14.5), facecolor='#f6f4ee')
    evidence = []
    for column, record in enumerate(records):
        name = record['controller']
        movie = next(r for r in manifest['recordings'] if r['controller'] == name)
        phases = record['first_clear_verification']['phases']

        def middle(predicate):
            index = next(i for i, phase in enumerate(phases) if predicate(phase))
            start = phases[index]['frame'] / 60
            end = phases[index + 1]['frame'] / 60 if index + 1 < len(phases) else start + 2
            return start + min(2, (end - start) / 2)

        points = [('hall', '名人堂统计', middle(lambda p: p['phase'][1] == 'PlayerStats')),
                  ('the-end', '片尾 THE END', middle(lambda p: p['phase'][2] == 'TheEnd')),
                  ('continued', '独立进程 CONTINUE 后', float(movie['probe']['format']['duration']) - 1 / 60)]
        for row, (key, label, second) in enumerate(points):
            output = args.media / f'{name}-{key}.png'
            subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin', '-y',
                            '-ss', f'{second:.9f}', '-i', str(args.media / movie['file']),
                            '-frames:v', '1', '-update', '1', str(output)], check=True)
            axes[row, column].imshow(plt.imread(output), interpolation='nearest')
            axes[row, column].axis('off')
            controller = '脚本（适配与恢复修复）' if name == 'script' else '双层 Jev 自主探索'
            axes[row, column].set_title(f'{controller}\n{label} · 原片 {int(second // 60)}:{int(second % 60):02}',
                                        color='#24312b', fontsize=14, pad=12)
            evidence.append({'controller': name, 'kind': key, 'file': output.name,
                             'source_video': movie['file'], 'source_sha256': movie['sha256'],
                             'source_s': second, 'process': 2 if key == 'continued' else 1})
    fig.suptitle('完整结局与独立读档：两条运行的真实画面', color='#24312b', fontsize=23, x=.08, ha='left', y=.992)
    fig.text(.08, .015, '截图用于定位画面；通关结论同时核对事件、自动存档和新进程读档断言。图片来自本次完整录制。',
             color='#52615a', fontsize=11)
    fig.subplots_adjust(left=.08, right=.97, top=.91, bottom=.05, hspace=.27, wspace=.13)
    fig.savefig(args.media / 'ending-proof.png', dpi=160, facecolor=fig.get_facecolor())
    plt.close(fig)
    (args.media / 'ending-frames.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
