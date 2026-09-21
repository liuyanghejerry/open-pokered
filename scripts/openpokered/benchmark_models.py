"""Versioned model configurations and a bounded JSON-lines adapter protocol.

The controller receives the same typed judgments for every provider. No fixture
labels, model credentials, or game protocol connection are sent to adapters.
"""
import json
import math
import os
import selectors
import subprocess
import time

from .typesafe import (Choice, Noul, Score, SystemOneResult, TypeSafeClient,
                       TypeSafeError, _parse_answer, DEFAULT_BASE_URL)

PROTOCOL = 'open-pokered-judge-v1'


def validate_model(config):
    common = {'id', 'provider', 'model', 'timeout_s'}
    provider = config.get('provider')
    extra = {
        'typesafe': {'api_key_env', 'base_url_env', 'max_retries'},
        'laya': {'revision', 'dtype', 'compile', 'cache_prompts', 'pad_to_multiple'},
        'command': {'command'},
    }
    if provider not in extra:
        raise ValueError(f'Unknown provider: {provider!r}')
    if set(config) - common - extra[provider]:
        raise ValueError(f'Unknown model configuration fields: {sorted(set(config) - common - extra[provider])}')
    for key in ('id', 'model'):
        if not isinstance(config.get(key), str) or not config[key].strip():
            raise ValueError(f'Model {key} must be a nonempty string')
    timeout = config.get('timeout_s', 30)
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not math.isfinite(timeout) or timeout <= 0:
        raise ValueError('timeout_s must be finite and positive')
    if provider == 'command':
        command = config.get('command')
        if not isinstance(command, list) or not command or not all(isinstance(a, str) and a for a in command):
            raise ValueError('command must be a nonempty argv array; no shell interpolation')
    if provider == 'laya':
        if not isinstance(config.get('revision'), str) or not config['revision']:
            raise ValueError('Laya requires an explicit checkpoint revision')
        if config.get('dtype', 'float16') not in ('float16', 'float32', 'bfloat16'):
            raise ValueError('Unsupported Laya dtype')
        for key in ('compile', 'cache_prompts'):
            if key in config and not isinstance(config[key], bool):
                raise ValueError(f'{key} must be boolean')
        padding = config.get('pad_to_multiple', 16)
        if isinstance(padding, bool) or not isinstance(padding, int) or padding < 1:
            raise ValueError('pad_to_multiple must be a positive integer')
    if provider == 'typesafe':
        retries = config.get('max_retries', 1)
        if isinstance(retries, bool) or not isinstance(retries, int) or not 0 <= retries <= 3:
            raise ValueError('max_retries must be an integer between 0 and 3')
        for key in ('api_key_env', 'base_url_env'):
            if key in config and (not isinstance(config[key], str) or not config[key].isidentifier()):
                raise ValueError(f'{key} must name an environment variable')
    return dict(config)


def question_from_json(value):
    kind = value['type']
    if kind == 'choice':
        return Choice(value['instructions'], value['criteria'])
    if kind == 'score':
        return Score(value['instructions'], value['criteria'])
    if kind == 'noul':
        return Noul(value['instructions'], **(value.get('criteria') or {}))
    raise ValueError(f'Unknown question type: {kind}')


def probability(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and 0 <= value <= 1


def validate_answers(answers, questions):
    if not isinstance(answers, dict) or set(answers) != set(questions):
        raise TypeSafeError('protocol_error: answer IDs must exactly match question IDs')
    for qid, question in questions.items():
        answer = answers[qid]
        if not isinstance(answer, dict) or answer.get('type') != question.TYPE:
            raise TypeSafeError('protocol_error: answer type mismatch')
        if question.TYPE == 'choice' and answer.get('choice') not in question.criteria:
            raise TypeSafeError('protocol_error: selected label is not a candidate')
        if question.TYPE == 'noul' and not probability(answer.get('noul')):
            raise TypeSafeError('protocol_error: invalid noul probability')
        if question.TYPE == 'score':
            score = answer.get('score')
            if isinstance(score, bool) or not isinstance(score, (int, float)) or not math.isfinite(score) or not 0 <= score <= len(question.criteria)-1:
                raise TypeSafeError('protocol_error: invalid score')
        if 'confidence' in answer and not probability(answer['confidence']):
            raise TypeSafeError('protocol_error: invalid confidence')
        distribution = answer.get('probabilities')
        if distribution:
            labels = (set(question.criteria) if question.TYPE == 'choice' else
                      {'false', 'true'} if question.TYPE == 'noul' else {str(i) for i in range(len(question.criteria))})
            if not isinstance(distribution, dict) or set(distribution) != labels or not all(probability(p) for p in distribution.values()):
                raise TypeSafeError('protocol_error: invalid probability labels or values')
            if abs(sum(distribution.values())-1) > max(.002, len(labels)*.0001):
                raise TypeSafeError('protocol_error: probabilities do not sum to one')


def usage_from_json(value):
    value = value or {}
    result = {}
    for key in ('input_tokens', 'output_tokens'):
        number = value.get(key)
        if number is not None and (isinstance(number, bool) or not isinstance(number, int) or number < 0):
            raise TypeSafeError('protocol_error: token counts must be nonnegative integers or null')
        result[key] = number
    return result


class UsageResponse:
    """Keep missing API usage missing; the legacy client otherwise defaults to 0."""
    def __init__(self, response, owner):
        self.response, self.owner = response, owner

    def __enter__(self):
        self.response.__enter__()
        return self

    def __exit__(self, *args):
        return self.response.__exit__(*args)

    def read(self):
        raw = self.response.read()
        self.owner.last_usage = usage_from_json(json.loads(raw).get('usage'))
        return raw


class TypeSafeAdapter:
    def __init__(self, config, opener):
        key_name = config.get('api_key_env', 'TYPESAFE_API_KEY')
        key = os.environ.get(key_name)
        if not key:
            raise TypeSafeError(f'Missing credential environment variable {key_name}')
        base = os.environ.get(config.get('base_url_env', 'TYPESAFE_BASE_URL')) or DEFAULT_BASE_URL
        self.model = config['model']
        self.last_usage = None
        self.metadata = {'provider': 'typesafe', 'requested_model': self.model,
                         'token_scope': 'Provider-reported usage; missing usage remains null.',
                         'rtt_policy': 'Measured TCP RTT per request attempt; capped by the run budget.'}
        self.client = TypeSafeClient(base, key, self.model, timeout=config.get('timeout_s', 30),
                                    max_retries=config.get('max_retries', 1),
                                    opener=lambda request, timeout: UsageResponse(opener(request, timeout), self))

    def system_one(self, state, questions, model=None):
        self.last_usage = None
        return self.client.system_one(state, questions, model=self.model)


class CommandAdapter:
    """Persistent adapter. stdout is protocol-only; stderr goes to the run log."""
    MAX_LINE = 8 * 1024 * 1024

    def __init__(self, config):
        self.model = config['model']
        self.timeout = config.get('timeout_s', 30)
        self.last_usage = None
        self.last_audit = []
        self.counter = 0
        self.buffer = bytearray()
        self.proc = subprocess.Popen(config['command'], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     bufsize=0)  # Inherits the benchmark's private process group.
        os.set_blocking(self.proc.stdout.fileno(), False)
        os.set_blocking(self.proc.stdin.fileno(), False)
        try:
            hello = self._read(time.monotonic() + self.timeout)
            if hello.get('protocol') != PROTOCOL or hello.get('ready') is not True:
                raise TypeSafeError('protocol_error: adapter must send a ready handshake')
            self.metadata = {'provider': 'command', 'requested_model': self.model,
                             'protocol': PROTOCOL, 'adapter_metadata': hello.get('metadata', {}),
                             'rtt_policy': 'No verified socket RTT; no deduction.',
                             'token_scope': 'Adapter-reported usage; missing usage remains null.'}
        except BaseException:
            self.close()
            raise

    def _wait(self, stream, event, deadline):
        remaining = deadline-time.monotonic()
        if remaining <= 0:
            raise TypeSafeError('timeout: adapter exceeded request deadline')
        with selectors.DefaultSelector() as selector:
            selector.register(stream, event)
            if not selector.select(remaining):
                raise TypeSafeError('timeout: adapter exceeded request deadline')

    def _read(self, deadline):
        while b'\n' not in self.buffer:
            self._wait(self.proc.stdout, selectors.EVENT_READ, deadline)
            block = os.read(self.proc.stdout.fileno(), 65536)
            if not block:
                raise TypeSafeError('protocol_error: adapter closed stdout before a response')
            self.buffer.extend(block)
            if len(self.buffer) > self.MAX_LINE:
                raise TypeSafeError('protocol_error: adapter response exceeds 8 MiB')
        line, _, rest = self.buffer.partition(b'\n')
        self.buffer = bytearray(rest)
        try:
            data = json.loads(line)
        except (ValueError, UnicodeError) as error:
            raise TypeSafeError('protocol_error: adapter returned invalid JSON') from error
        if not isinstance(data, dict):
            raise TypeSafeError('protocol_error: adapter response must be an object')
        return data

    def system_one(self, state, questions, model=None):
        self.last_usage = None
        self.last_audit = []
        self.counter += 1
        deadline = time.monotonic() + self.timeout
        request = {'protocol': PROTOCOL, 'id': self.counter, 'model': self.model, 'state': state,
                   'questions': {key: q.to_json() for key, q in questions.items()}}
        payload = memoryview((json.dumps(request, ensure_ascii=False) + '\n').encode())
        try:
            while payload:
                self._wait(self.proc.stdin, selectors.EVENT_WRITE, deadline)
                payload = payload[os.write(self.proc.stdin.fileno(), payload):]
            response = self._read(deadline)
            if response.get('id') != self.counter:
                raise TypeSafeError('protocol_error: response request ID mismatch')
            if response.get('error'):
                raise TypeSafeError('adapter_error: ' + str(response['error'])[:500])
            self.last_usage = usage_from_json(response.get('usage'))
            validate_answers(response.get('answers'), questions)
            actual_model = response.get('model')
            if not isinstance(actual_model, str) or not actual_model:
                raise TypeSafeError('protocol_error: response must identify the actual model')
            return SystemOneResult(actual_model, {k: _parse_answer(a) for k, a in response['answers'].items()},
                                   self.last_usage['input_tokens'] or 0, self.last_usage['output_tokens'] or 0)
        except (BrokenPipeError, OSError) as error:
            raise TypeSafeError('adapter_transport_error: ' + str(error)) from error

    def close(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self.proc.stdin.close()
        self.proc.stdout.close()


def create_adapter(config):
    from .evaluation_models import LayaClient, RttOpener
    config = validate_model(config)
    if config['provider'] == 'typesafe':
        opener = RttOpener()
        return TypeSafeAdapter(config, opener), opener
    if config['provider'] == 'laya':
        options = {k: config[k] for k in ('dtype', 'compile', 'cache_prompts', 'pad_to_multiple') if k in config}
        return LayaClient(config['model'], config['revision'], **options), None
    return CommandAdapter(config), None
