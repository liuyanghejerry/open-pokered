"""TypeSafe System One (Jev) adapter — typed judgments instead of prompt-and-parse.

System One models return *judgments* (a choice, a probability, a graded
score) rather than generated text, so the answer is directly consumable
and there is no parse step to fail. This module is the transport: typed
question builders, typed answers, credential resolution, bounded retries.

Two call sites in this repo use it (see `semantics.py`):

- goal -> entity targeting, where the previous code matched an NPC name
  with a substring test and could not handle a paraphrase.
- dialogue assertions in tests, where exact-substring matching breaks on
  harmless reflow or wording changes.

Stdlib only (`urllib`), matching the `llm_agent.ChatClient` idiom.
Credentials resolve from `TYPESAFE_API_KEY`; `from_env` also reads a
repo-root `.env` first, so a local key works without exporting it.
`.env` is gitignored, never commit it.

Every caller degrades to its previous deterministic behaviour when no key
is configured, so offline and CI runs are unchanged.
"""
import json
import os
import time
import urllib.error
import urllib.request
from pathlib import Path

API_KEY_ENV = "TYPESAFE_API_KEY"
BASE_URL_ENV = "TYPESAFE_BASE_URL"
MODEL_ENV = "TYPESAFE_DEFAULT_MODEL"
DEFAULT_BASE_URL = "https://api.typesafe.ai"
DEFAULT_MODEL = "jev-latest"
SYSTEM_ONE_PATH = "/v1/systemone"

REPO_ROOT = Path(__file__).resolve().parents[2]


class CredentialError(RuntimeError):
    pass


class TypeSafeError(RuntimeError):
    """HTTP/transport failure after the bounded retry sequence."""


# ── credentials ───────────────────────────────────────────────────────
def load_env_file(path=None, env=None):
    """Fill missing keys from a `.env` file; never override a real env var.

    Returns the number of keys added. A missing file is not an error —
    CI has no `.env` and must keep working.
    """
    env = os.environ if env is None else env
    path = REPO_ROOT / ".env" if path is None else Path(path)
    try:
        text = path.read_text()
    except OSError:
        return 0
    added = 0
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        if key.startswith("export "):
            key = key[len("export "):].strip()
        value = value.strip().strip("'\"")
        if key and key not in env:
            env[key] = value
            added += 1
    return added


def resolve_credentials(env=None):
    """(base_url, api_key, source) — the key is required, the rest default."""
    env = os.environ if env is None else env
    key = env.get(API_KEY_ENV)
    if not key:
        raise CredentialError(
            f"no TypeSafe credentials: set {API_KEY_ENV} (or put it in .env)")
    base = env.get(BASE_URL_ENV) or DEFAULT_BASE_URL
    return base.rstrip("/"), key, "typesafe"


def default_model(env=None):
    env = os.environ if env is None else env
    return env.get(MODEL_ENV) or DEFAULT_MODEL


def configured(env=None):
    """True when a key is available, i.e. semantic calls can be made.

    `.env` is consulted only for the process environment. An explicitly
    passed mapping is taken as complete, so a caller can exercise the
    unconfigured path without picking up the developer's own key.
    """
    if env is None:
        env = os.environ
        load_env_file(env=env)
    return bool(env.get(API_KEY_ENV))


# ── typed questions ───────────────────────────────────────────────────
class Question:
    """One typed judgment. `instructions` carries the question itself —
    ids are for code and are never sent to the model, so the question
    must be self-contained."""

    TYPE = ""

    def __init__(self, instructions, criteria=None):
        self.instructions = instructions
        self.criteria = criteria

    def to_json(self):
        q = {"type": self.TYPE, "instructions": self.instructions}
        if self.criteria is not None:
            q["criteria"] = self.criteria
        return q


class Noul(Question):
    """Whether a condition holds. Returns P(yes) on 0..1.

    Use one Noul per label when several conditions may hold at once; a
    single Noul near 0.5 means the alternatives are equally likely, not
    that the answer is "medium".
    """

    TYPE = "noul"

    def __init__(self, instructions, true=None, false=None):
        criteria = None
        if true is not None or false is not None:
            criteria = {"true": true, "false": false}
        super().__init__(instructions, criteria)


class Choice(Question):
    """One option out of a defined set, plus the full distribution.

    The probability mass compares competing options; `confidence`
    summarises how concentrated the distribution is, which is a
    different thing from whether the choice is correct.
    """

    TYPE = "choice"

    def __init__(self, instructions, criteria):
        if not criteria:
            raise ValueError("a Choice needs at least one option")
        super().__init__(instructions, dict(criteria))


class Score(Question):
    """Degree along ordered levels. Returns the probability-weighted
    position, so the value can land between levels."""

    TYPE = "score"

    def __init__(self, instructions, criteria):
        if len(criteria) < 2:
            raise ValueError("a Score needs at least two levels")
        super().__init__(instructions, list(criteria))


# ── typed answers ─────────────────────────────────────────────────────
class Answer:
    pass


class NoulAnswer(Answer):
    def __init__(self, noul):
        self.noul = noul

    def __repr__(self):
        return f"NoulAnswer({self.noul:.3f})"


class ChoiceAnswer(Answer):
    def __init__(self, choice, probabilities, confidence):
        self.choice = choice
        self.probabilities = probabilities
        self.confidence = confidence

    def __repr__(self):
        return f"ChoiceAnswer({self.choice!r}, conf={self.confidence:.3f})"


class ScoreAnswer(Answer):
    def __init__(self, score, legend, probabilities, confidence):
        self.score = score
        self.legend = legend
        self.probabilities = probabilities
        self.confidence = confidence

    def __repr__(self):
        return f"ScoreAnswer({self.score:.3f}, conf={self.confidence:.3f})"


class SystemOneResult:
    def __init__(self, model, answers, input_tokens, output_tokens):
        self.model = model
        self.answers = answers
        self.input_tokens = input_tokens
        self.output_tokens = output_tokens


def _parse_answer(raw):
    kind = raw.get("type")
    if kind == "noul":
        return NoulAnswer(raw["noul"])
    if kind == "choice":
        return ChoiceAnswer(raw["choice"], raw.get("probabilities") or {},
                            raw.get("confidence", 0.0))
    if kind == "score":
        return ScoreAnswer(raw["score"], raw.get("legend") or {},
                           raw.get("probabilities") or {},
                           raw.get("confidence", 0.0))
    raise TypeSafeError(f"unknown answer type {kind!r}")


# ── HTTP client ───────────────────────────────────────────────────────
class TypeSafeClient:
    """Minimal System One client.

    `opener` is injectable for tests: called with a urllib Request, must
    return a context-manager response whose .read() yields the JSON body
    (same seam as `llm_agent.ChatClient`).
    """

    # 529 is TypeSafe's "overloaded"; both are transient.
    RETRYABLE = (429, 529)

    def __init__(self, base_url, api_key, model, timeout=10.0,
                 max_retries=2, opener=None):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.model = model
        self.timeout = timeout
        self.max_retries = max_retries
        self.opener = opener or urllib.request.urlopen

    @classmethod
    def from_env(cls, env=None, **kw):
        if env is None:
            env = os.environ
            load_env_file(env=env)
        base, key, _source = resolve_credentials(env)
        return cls(base, key, default_model(env), **kw)

    def system_one(self, state, questions, model=None):
        """Evaluate `state` against `questions`; one answer per id."""
        if not questions:
            raise ValueError("no questions asked")
        body = json.dumps({
            "state": state,
            "model": model or self.model,
            "questions": {qid: q.to_json() for qid, q in questions.items()},
        }).encode()
        req = urllib.request.Request(
            self.base_url + SYSTEM_ONE_PATH, data=body,
            headers={"Authorization": f"Bearer {self.api_key}",
                     "Content-Type": "application/json"})
        last_err = None
        for attempt in range(self.max_retries + 1):
            try:
                with self.opener(req, timeout=self.timeout) as r:
                    data = json.loads(r.read())
                usage = data.get("usage") or {}
                return SystemOneResult(
                    data.get("model", ""),
                    {qid: _parse_answer(raw)
                     for qid, raw in (data.get("answers") or {}).items()},
                    usage.get("input_tokens", 0),
                    usage.get("output_tokens", 0))
            except urllib.error.HTTPError as e:
                detail = e.read()[:200] if hasattr(e, "read") else b""
                last_err = TypeSafeError(f"HTTP {e.code}: {detail!r}")
                # A 4xx that is not a rate limit is a request problem:
                # retrying the same body cannot help.
                if e.code not in self.RETRYABLE and 400 <= e.code < 500:
                    raise last_err
            except Exception as e:  # network/parse — retryable
                last_err = TypeSafeError(f"{type(e).__name__}: {e}")
            if attempt < self.max_retries:
                time.sleep(0.5 * (attempt + 1))
        raise last_err


def judge(state, questions, client=None, env=None):
    """One-shot helper: build the client from the environment if needed."""
    client = client or TypeSafeClient.from_env(env)
    return client.system_one(state, questions)
