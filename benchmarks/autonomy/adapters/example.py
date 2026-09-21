#!/usr/bin/env python3
"""Runnable protocol example, NOT an intelligent model or benchmark baseline.

Replace infer() with your provider SDK/local model. Load weights before emitting
ready. Keep stdout for JSON-lines, and send diagnostics to stderr. Credentials
belong in environment variables, never in the benchmark manifest.
"""
import json
import sys


def infer(state, questions, model):
    # Intentionally trivial: exercise transport and scoring without paid APIs.
    # Do not use ground-truth fixtures here; the adapter receives no labels.
    answers = {}
    for qid, question in questions.items():
        if question['type'] != 'choice':
            raise ValueError('Example adapter implements choice only')
        answers[qid] = {'type': 'choice', 'choice': next(iter(question['criteria']))}
    return {'model': 'example-first-option-stub', 'answers': answers,
            'usage': {'input_tokens': None, 'output_tokens': None}}


def main():
    print(json.dumps({'protocol': 'open-pokered-judge-v1', 'ready': True,
                      'metadata': {'implementation': 'example-first-option', 'stub': True}}), flush=True)
    for line in sys.stdin:
        request = json.loads(line)
        try:
            result = infer(request['state'], request['questions'], request['model'])
            response = {'id': request['id'], **result}
        except Exception as error:
            response = {'id': request['id'], 'error': f'{type(error).__name__}: {error}'}
        print(json.dumps(response), flush=True)


if __name__ == '__main__':
    main()
