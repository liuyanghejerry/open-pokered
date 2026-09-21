#!/usr/bin/env python3
"""Plot measured full-playthrough evidence; requires matplotlib."""
import argparse
import collections
import json
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib import font_manager
import numpy as np


def draw_navigation(source, output, font='/Library/Fonts/Arial Unicode.ttf'):
    from matplotlib import colors, patches
    data = json.loads(Path(source).read_text())
    font_manager.fontManager.addfont(font)
    plt.rcParams.update({'font.family': font_manager.FontProperties(fname=font).get_name(),
                         'svg.fonttype': 'path'})
    grid = np.zeros((data['height'], data['width']))
    for value, key in [(1, 'walkable'), (2, 'player_component'), (3, 'target_component')]:
        for x, y in data[key]:
            grid[y, x] = value
    palette = ['#f6f4ee', '#d8ded7', '#6085a3', '#dba95a']
    fig, ax = plt.subplots(figsize=(9.2, 7.6), facecolor=palette[0])
    ax.imshow(grid, cmap=colors.ListedColormap(palette), vmin=0, vmax=3, interpolation='nearest')
    for label, key, marker, text_at in [
            ('失败时所在位置', 'player', 'o', (12, 4)),
            ('脚本要去的楼梯', 'target', '*', (9, 21))]:
        x, y = data[key]
        ax.scatter([x], [y], s=140, marker=marker, c='#24312b', edgecolors='white', linewidths=1, zorder=3)
        ax.annotate(f'{label}\n({x}, {y})', (x, y), xytext=text_at, textcoords='data', fontsize=12,
                    color='#24312b', arrowprops={'arrowstyle': '->', 'color': '#24312b'},
                    bbox={'boxstyle': 'round,pad=.5', 'fc': palette[0], 'ec': 'none'})
    ax.set_xticks(range(0, 28, 4)); ax.set_yticks(range(0, 28, 4))
    ax.set_xlabel('地图格坐标 x'); ax.set_ylabel('地图格坐标 y')
    for spine in ax.spines.values():
        spine.set_visible(False)
    ax.legend(handles=[patches.Patch(color=palette[2], label='当前位置所在通道'),
                       patches.Patch(color=palette[3], label='目标楼梯所在通道')], loc='upper left')
    fig.suptitle('月见山失败：起点与目标位于不同通道', x=.08, ha='left', fontsize=20, color='#24312b')
    fig.text(.08, .025, '来源：script-a4 的失败坐标与冻结版本地形。即使忽略额外障碍，两点仍不能在该层直接步行相通。',
             fontsize=10, color='#52615a')
    fig.subplots_adjust(left=.1, right=.95, top=.9, bottom=.12)
    for ext in ('png', 'svg'):
        fig.savefig(Path(output) / f'script-moon-navigation.{ext}', dpi=160, facecolor=palette[0])
    plt.close(fig)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--analysis', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    ap.add_argument('--font', default='/Library/Fonts/Arial Unicode.ttf')
    args = ap.parse_args()
    font_manager.fontManager.addfont(args.font)
    plt.rcParams.update({'font.family': font_manager.FontProperties(fname=args.font).get_name(),
                         'axes.unicode_minus': False, 'font.size': 12,
                         'figure.facecolor': '#f6f4ee', 'axes.facecolor': '#f6f4ee',
                         'text.color': '#24312b', 'axes.labelcolor': '#24312b',
                         'xtick.color': '#52615a', 'ytick.color': '#52615a',
                         'axes.spines.top': False, 'axes.spines.right': False,
                         'axes.spines.left': False, 'axes.spines.bottom': False,
                         'svg.fonttype': 'path'})
    records = json.loads(args.analysis.read_text())
    assert all(r['finished'] and r['success'] for r in records)
    args.output.mkdir(parents=True, exist_ok=True)
    colors = {'script': '#536d8d', 'jev': '#237451'}
    labels = {'script': '脚本（适配与恢复修复）', 'jev': '双层 Jev 自主探索'}

    def save(fig, name):
        for ext in ('png', 'svg'):
            fig.savefig(args.output / f'{name}.{ext}', dpi=170, facecolor=fig.get_facecolor())
        plt.close(fig)

    fig, ax = plt.subplots(figsize=(11.5, 5.8))
    for record in records:
        name = record['controller']
        x = [0] + [b['wall_s'] / 60 for b in record['badges']] + [record['wall_s'] / 60]
        y = [0] + [int(b['badges']).bit_count() for b in record['badges']] + [9]
        ax.step(x, y, where='post', color=colors[name], linewidth=2.5, label=labels[name])
        ax.scatter(x[1:], y[1:], color=colors[name], s=30, zorder=3)
        ax.annotate(f"{record['wall_s'] / 60:.1f} 分钟", (x[-1], 9),
                    xytext=(0, 12 if name == 'script' else -22), textcoords='offset points',
                    ha='center', color=colors[name])
    ax.set_yticks(range(10), ['开局'] + [f'{i} 枚徽章' for i in range(1, 9)] + ['通关验证'])
    ax.set_ylim(-.2, 9.7)
    ax.set_xlabel('从本次 NEW GAME 启动起的墙钟时间（分钟）')
    ax.grid(axis='y', alpha=.15)
    ax.legend(loc='lower right', frameon=False)
    fig.suptitle('相同主线终点，推进节奏不同', x=.09, y=.98, ha='left', fontsize=20)
    fig.text(.09, .045, '包括录制开销；默认时间模式不同且共享主机负载，仅描述这对运行，不作为模型速度的因果结论。', fontsize=10, color='#52615a')
    fig.subplots_adjust(left=.13, right=.94, top=.87, bottom=.17)
    save(fig, 'full-progress')

    fig, ax = plt.subplots(figsize=(11.5, 5.8))
    gym_names = ['小刚', '小霞', '马志士', '莉佳', '阿桔', '娜姿', '夏伯', '坂木']
    xs = np.arange(8)
    for record in records:
        name = record['controller']
        levels, order = [0] * 8, [0] * 8
        old = 0
        for i, badge in enumerate(record['badges']):
            bit = (badge['badges'] ^ old).bit_length() - 1
            levels[bit] = badge['party'][0]['level']
            order[bit] = i + 1
            old = badge['badges']
        pos = xs + (-.19 if name == 'script' else .19)
        bars = ax.bar(pos, levels, width=.34, color=colors[name], label=labels[name])
        for bar, level, index in zip(bars, levels, order):
            ax.text(bar.get_x() + bar.get_width()/2, level + 1,
                    f'{level}\n第 {index} 站', ha='center', va='bottom', fontsize=10, color=colors[name])
    ax.set_xticks(xs, gym_names)
    ax.set_ylabel('取得徽章时队伍首位精灵的等级')
    ax.set_ylim(0, max(p['party'][0]['level'] for r in records for p in r['badges']) + 15)
    ax.grid(axis='y', alpha=.15)
    ax.legend(frameon=False, loc='upper left')
    fig.suptitle('训练投入与道馆顺序：不能只看是否获胜', x=.09, y=.98, ha='left', fontsize=20)
    fig.text(.09, .045, '数字为胜利后的观察值，包含本场经验；“第几站”表示实际徽章顺序。精灵种类与招式差异影响挑战难度。', fontsize=10, color='#52615a')
    fig.subplots_adjust(left=.09, right=.96, top=.87, bottom=.16)
    save(fig, 'full-badge-levels')

    counts = {r['controller']: collections.Counter(
        p['map_name'] for p in r['observed_party_wipe_episodes']
        if 'player_won: false' in p['battle_phase']) for r in records}
    combined = sum(counts.values(), collections.Counter())
    areas = [name for name, _ in combined.most_common(10)]
    if len(combined) > len(areas):
        for name, counter in counts.items():
            counter['其他地点'] = sum(v for k, v in counter.items() if k not in areas)
        areas.append('其他地点')
    translated = {'OaksLab': '大木研究所', 'ViridianForest': '常青森林', 'PewterGym': '尼比道馆',
                  'MtMoonB2F': '月见山 B2F', 'CeruleanGym': '华蓝道馆', 'CeruleanCity': '华蓝市',
                  'LoreleisRoom': '科拿房间', 'LancesRoom': '渡房间', 'BrunosRoom': '希巴房间',
                  'AgathasRoom': '菊子房间', 'ChampionsRoom': '冠军房间', 'SSAnne2F': '圣安奴号 2F'}
    fig, ax = plt.subplots(figsize=(11.5, max(5.5, len(areas) * .48 + 1.6)))
    ys = np.arange(len(areas))
    for name, offset in [('script', -.18), ('jev', .18)]:
        values = [counts[name][place] for place in areas]
        ax.barh(ys + offset, values, height=.32, color=colors[name], label=labels[name])
        for y, value in zip(ys + offset, values):
            if value:
                ax.text(value + .15, y, str(value), va='center', fontsize=11, color=colors[name])
    ax.set_yticks(ys, [translated.get(place, place) for place in areas])
    ax.invert_yaxis()
    ax.set_xlim(0, max((max(c.values(), default=0) for c in counts.values()), default=0) + 3)
    ax.set_xlabel('观察到全队 HP 归零且明确结算败退的次数')
    ax.legend(frameon=False, loc='lower right')
    ax.grid(axis='x', alpha=.15)
    fig.suptitle('失败集中在哪里，决定了恢复策略的价值', x=.09, y=.98, ha='left', fontsize=20)
    fig.text(.09, .045, '按观测战斗片段去重；排除阿桔自爆导致双方归零但判胜的事件。道馆地点也包含普通训练家。', fontsize=10, color='#52615a')
    fig.subplots_adjust(left=.18, right=.94, top=.87, bottom=.16)
    save(fig, 'full-defeat-locations')
    if (args.output / 'script-moon-navigation.json').exists():
        draw_navigation(args.output / 'script-moon-navigation.json', args.output, args.font)


if __name__ == '__main__':
    main()
