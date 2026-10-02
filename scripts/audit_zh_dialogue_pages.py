#!/usr/bin/env python3
"""Independently check Chinese word boundaries in a core-test page dump.

Development dependency: jieba==0.42.1. The Rust corpus test checks pixels,
punctuation, preservation and runtime names; this pass segments entire source
sentences independently of the runtime's greedy protected-word lookup.
"""
import argparse
import json
from pathlib import Path
import re


def compact(text):
    return re.sub(r'\s+', '', text)


def main():
    import jieba
    if jieba.__version__ != '0.42.1':
        raise SystemExit('Use jieba==0.42.1 for reproducible auditing')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pages', type=Path)
    parser.add_argument('--before', type=Path)
    parser.add_argument('--transcript', type=Path, help='write all prepared pages as JSONL')
    args = parser.parse_args()
    rows = json.loads(args.pages.read_text())
    jieba.setLogLevel(40)
    tokenizer = jieba.Tokenizer()
    for word in '宝可梦 训练家 精灵球 火箭队 宝可梦中心 大木博士 小智 小茂 妙蛙种子'.split():
        tokenizer.add_word(word, 1000000)
    candidates = []
    for row in rows:
        source = compact(row['text'])
        spans = []
        position = 0
        for word in tokenizer.cut(source, HMM=False):
            spans.append((position, position + len(word), word))
            position += len(word)
        offset = 0
        for page_index, page in enumerate(row['pages']):
            for field in sorted(page):
                offset += len(compact(page[field]))
                for start, end, word in spans:
                    if start < offset < end and re.fullmatch(r'[\u3400-\u9fff]{2,}', word):
                        candidates.append({'source': row['source'], 'page': page_index,
                                           'row': field, 'word': word, 'text': page[field]})
    summary = {'records': len(rows), 'pages': sum(len(r['pages']) for r in rows),
               'blank_pages': sum(not compact(''.join(p.values()))
                                  for r in rows for p in r['pages']),
               'word_boundary_candidates': candidates}
    if args.before:
        before = json.loads(args.before.read_text())
        assert [r['source'] for r in rows] == [r['source'] for r in before]
        assert [r['text'] for r in rows] == [r['text'] for r in before]
        summary['before_pages'] = sum(len(r['pages']) for r in before)
        summary['before_blank_pages'] = [
            {'source': r['source'], 'page': i}
            for r in before for i, p in enumerate(r['pages'])
            if not compact(''.join(p.values()))]
    if args.transcript:
        args.transcript.write_text(''.join(
            json.dumps(row, ensure_ascii=False, separators=(',', ':')) + '\n'
            for row in rows))
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    if candidates or summary['blank_pages']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
