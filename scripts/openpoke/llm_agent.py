"""LLM agent adapter for the RQ1/RQ2 experiments (WP1).

Three tier policies driving the same seeded headless game through an
OpenAI-compatible chat-completions endpoint — no third-party deps
(stdlib `urllib` only), matching the `policies.py` interface so runs are
comparable with the calibration baselines:

- `ButtonLlmAgent` (T1): low-level observation (ObservationLevel 1) +
  controller buttons only (`drive:<btn>,N`).
- `SkillLlmAgent` (T2): full symbolic observation (nearby entities) +
  skill-level actions (`move_to` / `interact` / `interact_with` /
  `travel_to`), battle delegated to `skills.fight_current_battle` like
  the calibration T2 (battle handling is orthogonal to the nav
  abstraction).
- `WorldModelLlmAgent` (T3): T2 observation + world-graph route legs
  (M3) and script-semantics hints (M4) in the prompt.

Credential resolution order (first match wins):
1. `OPENAI_BASE_URL` + `OPENAI_API_KEY` (any OpenAI-compatible endpoint)
2. `HF_TOKEN` (HuggingFace Inference Providers router,
   base `https://router.huggingface.co/v1`)

Default model: `Qwen/Qwen2.5-Coder-3B-Instruct` — the smallest instruct
model enabled on the HF router for the probe token (verified with a tiny
chat completion); override with `OPENPOKE_MODEL`.

Parse failures are bounded: one corrective retry per decision, then a
recorded deterministic fallback action (`parse_failures` /
`degraded_actions` counters land in `RunMetrics.extra`). All runs use
`--speed 0` (driven-only) via the caller, like the calibration matrix.
"""
import json
import os
import re
import time
import urllib.error
import urllib.request

from . import skills

HF_BASE_URL = "https://router.huggingface.co/v1"
DEFAULT_MODEL = "Qwen/Qwen2.5-Coder-3B-Instruct"
MODEL_ENV = "OPENPOKE_MODEL"


class CredentialError(RuntimeError):
    pass


class LlmError(RuntimeError):
    """HTTP/transport failure after the bounded retry sequence."""


def resolve_credentials(env=None):
    """(base_url, api_key, source) per the documented resolution order."""
    env = os.environ if env is None else env
    base = env.get("OPENAI_BASE_URL")
    key = env.get("OPENAI_API_KEY")
    if base and key:
        return base.rstrip("/"), key, "openai"
    token = env.get("HF_TOKEN")
    if token:
        return HF_BASE_URL, token, "huggingface"
    raise CredentialError(
        "no LLM credentials: set OPENAI_BASE_URL+OPENAI_API_KEY or HF_TOKEN")


def default_model(env=None):
    env = os.environ if env is None else env
    return env.get(MODEL_ENV) or DEFAULT_MODEL


# ── HTTP client ───────────────────────────────────────────────────────
class ChatResult:
    def __init__(self, text, prompt_tokens, completion_tokens):
        self.text = text
        self.prompt_tokens = prompt_tokens
        self.completion_tokens = completion_tokens


class ChatClient:
    """Minimal OpenAI-compatible chat-completions client.

    `opener` is injectable for tests: called with a urllib Request, must
    return a context-manager response whose .read() yields the JSON body.
    """

    def __init__(self, base_url, api_key, model, timeout=60, max_retries=2,
                 opener=None):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.model = model
        self.timeout = timeout
        self.max_retries = max_retries
        self.opener = opener or urllib.request.urlopen

    def chat(self, messages, max_tokens=64):
        body = json.dumps({
            "model": self.model,
            "messages": messages,
            "max_tokens": max_tokens,
        }).encode()
        req = urllib.request.Request(
            self.base_url + "/chat/completions", data=body,
            headers={"Authorization": f"Bearer {self.api_key}",
                     "Content-Type": "application/json"})
        last_err = None
        for attempt in range(self.max_retries + 1):
            try:
                with self.opener(req, timeout=self.timeout) as r:
                    data = json.loads(r.read())
                choice = data["choices"][0]["message"]
                usage = data.get("usage") or {}
                return ChatResult(
                    choice.get("content") or "",
                    usage.get("prompt_tokens", 0),
                    usage.get("completion_tokens", 0))
            except urllib.error.HTTPError as e:
                detail = e.read()[:200] if hasattr(e, "read") else b""
                last_err = LlmError(f"HTTP {e.code}: {detail!r}")
                # 4xx (other than 429) is a request problem — do not retry.
                if e.code != 429 and 400 <= e.code < 500:
                    raise last_err
            except Exception as e:  # network/parse — retryable
                last_err = LlmError(f"{type(e).__name__}: {e}")
            if attempt < self.max_retries:
                time.sleep(0.5 * (attempt + 1))
        raise last_err


# ── action parsing ────────────────────────────────────────────────────
_BUTTONS = ("a", "b", "up", "down", "left", "right", "start", "select")

# Replies are full-string matches: the prompt demands exactly one action
# and nothing else, so prose around the action is a parse failure (and
# triggers the corrective retry).
_T1_RE = re.compile(
    r"(?:ACTION\s*:\s*)?(a|b|up|down|left|right|start|select)"
    r"(?:\s*[x×*]?\s*(\d{1,2}))?\s*$", re.IGNORECASE)
_T2_RE = re.compile(
    r"(?:ACTION\s*:\s*)?"
    r"(move_to:\s*\d{1,3}\s*,\s*\d{1,3}|travel_to:\s*[A-Za-z0-9]+|"
    r"interact_with:\s*[a-z]+:\d+|interact|press:\s*[a-z]+|step_frames:\s*\d{1,4})"
    r"\s*$", re.IGNORECASE)


class ParseFailure(ValueError):
    pass


def parse_t1_action(text):
    """"up", "ACTION: up x8", "a" → drive/press verb string."""
    m = _T1_RE.match(text.strip())
    if not m:
        raise ParseFailure(f"no button in {text!r}")
    btn = m.group(1).lower()
    n = int(m.group(2)) if m.group(2) else 1
    n = max(1, min(16, n))
    if btn in ("up", "down", "left", "right"):
        return f"drive:{btn},{max(4, n)}"
    return f"press:{btn}" if n == 1 else f"drive:{btn},{n}"


def parse_t2_action(text):
    m = _T2_RE.match(text.strip())
    if not m:
        raise ParseFailure(f"no skill action in {text!r}")
    action = re.sub(r"\s+", "", m.group(1))
    verb = action.split(":", 1)[0].lower()
    rest = action.split(":", 1)[1] if ":" in action else ""
    if verb == "move_to":
        x, y = (int(v) for v in rest.split(","))
        return f"move_to:{x},{y}"
    if verb == "travel_to":
        return f"travel_to:{rest}"
    if verb == "interact_with":
        return f"interact_with:{rest.lower()}"
    if verb == "interact":
        return "interact"
    if verb == "press":
        btn = rest.lower()
        if btn not in _BUTTONS:
            raise ParseFailure(f"unknown button {btn!r}")
        return f"press:{btn}"
    if verb == "step_frames":
        return f"step_frames:{max(1, min(600, int(rest)))}"
    raise ParseFailure(f"unknown verb {verb!r}")


# ── shared policy machinery ───────────────────────────────────────────
class LlmAgent:
    """Base: observation → model → parse → env.step loop with bounded
    corrective retry + deterministic fallback on parse failure."""

    POLICY_NAME = "llm_base"
    TIER = ""

    def __init__(self, client, seed, max_model_calls=60, history=8):
        self.client = client          # ChatClient
        self.seed = seed
        self.max_model_calls = max_model_calls
        self.history = history        # exchanges kept for continuity
        self.messages = []
        # accounting → RunMetrics
        self.model_calls = 0
        self.prompt_tokens = 0
        self.completion_tokens = 0
        self.parse_failures = 0
        self.degraded_actions = 0
        self.battles = 0
        self.battles_won = 0
        self._in_battle = False

    # ── tier hooks ────────────────────────────────────────────────────
    def system_prompt(self, task):
        raise NotImplementedError

    def describe(self, env, task, obs):
        """User-message observation text for this decision."""
        raise NotImplementedError

    def parse_action(self, text):
        raise NotImplementedError

    def fallback_action(self, obs):
        """Deterministic degrade after a failed parse + retry."""
        return "step_frames:10"

    # ── decision ──────────────────────────────────────────────────────
    def decide(self, env, task, obs):
        if self.model_calls >= self.max_model_calls:
            raise LlmError("model_call_cap")
        if not self.messages:
            self.messages.append({"role": "system",
                                  "content": self.system_prompt(task)})
        self.messages.append({"role": "user",
                              "content": self.describe(env, task, obs)})
        if len(self.messages) > 1 + 2 * self.history:
            del self.messages[1:3]
        text = None
        for attempt in range(2):  # one corrective retry, then degrade
            result = self.client.chat(self.messages)
            self.model_calls += 1
            self.prompt_tokens += result.prompt_tokens
            self.completion_tokens += result.completion_tokens
            text = result.text
            try:
                action = self.parse_action(text)
                self.messages.append({"role": "assistant", "content": text})
                return action, False
            except ParseFailure as e:
                self.parse_failures += 1
                if attempt == 0:
                    self.messages.append({"role": "assistant", "content": text})
                    self.messages.append({
                        "role": "user",
                        "content": f"Invalid reply ({e}). Answer with exactly "
                                   f"one action in the required format, nothing else."})
        self.degraded_actions += 1
        self.messages.append({"role": "assistant", "content": text or ""})
        return self.fallback_action(obs), True

    # ── main loop ─────────────────────────────────────────────────────
    def run(self, env, task, frame_budget):
        start = env.frame_count()
        obs = env.client.observe()
        while True:
            if env.frame_count() - start > frame_budget:
                return False, "frame_budget"
            mode = obs["mode"]
            if mode == "battle":
                if not self._in_battle:
                    self.battles += 1
                    self._in_battle = True
                obs, outcome, done = self._battle(env)
                if done:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue
            if mode in ("dialogue", "transition", "menu"):
                obs, outcome, done = self._settle(env)
                if done:
                    return outcome["success"], "" if outcome["success"] else "max_steps"
                continue
            try:
                action, _degraded = self.decide(env, task, obs)
            except LlmError as e:
                # A capped or failing model is a run outcome, not a crash.
                return False, f"{type(e).__name__}: {e}"
            obs, outcome, _ = env.step(action)
            if outcome["done"]:
                return outcome["success"], "" if outcome["success"] else "max_steps"

    def _battle(self, env):
        """T2/T3: delegate to the battle skill (orthogonal to the nav
        abstraction, same convention as the calibration tiers)."""
        skills.fight_current_battle(env.client, prefer="fight")
        self.battles_won += 1
        self._in_battle = False
        return env.client.observe(), {"done": False, "success": False}, False

    def _settle(self, env):
        env.client.skip_dialogue()
        obs, outcome, _ = env.step("step_frames:10")
        return obs, outcome, outcome["done"]


def _goal_text(task):
    goal = task["goal"]
    if goal["type"] == "map":
        return f"reach the map {goal['id']}"
    if goal["type"] == "item":
        return f"obtain the item {goal['id']}"
    if goal["type"] == "flag":
        return f"achieve the story objective '{task['name']}'"
    if goal["type"] == "battle_won":
        return "win a wild Pokémon battle"
    if goal["type"] == "party_count":
        return f"have at least {goal['min']} Pokémon in the party"
    return task["name"]


def _nearby_text(env, limit=12):
    entities = env.client.nearby().get("entities", [])[:limit]
    if not entities:
        return "nearby: (nothing detected)"
    parts = []
    for e in entities:
        pos = e.get("position") or {}
        name = e.get("name") or "?"
        parts.append(f"{e['id']}({e.get('kind')},{name},"
                     f"x{pos.get('x')},y{pos.get('y')},d{e.get('distance')})")
    return "nearby: " + " ".join(parts)


# ── T1: buttons + low-level observation ──────────────────────────────
class ButtonLlmAgent(LlmAgent):
    POLICY_NAME = "llm_buttons"
    TIER = "T1"

    def system_prompt(self, task):
        return (
            "You play Pokémon Red (Game Boy) with a controller. Goal: "
            f"{_goal_text(task)}. Each turn I report the game state and "
            "you answer with EXACTLY ONE button press in the form "
            "`up xN` / `down xN` / `left xN` / `right xN` (N = 1-16 tiles) "
            "or `a` / `b` / `start` / `select` (single press). No prose, "
            "no explanation — just the button.")

    def describe(self, env, task, obs):
        pos = obs["position"]
        mode = obs["mode"]
        return (f"map={obs['map']['name']} x={pos['x']} y={pos['y']} "
                f"facing={obs.get('facing')} mode={mode}. "
                f"Goal: {_goal_text(task)}. Which button?")

    def parse_action(self, text):
        return parse_t1_action(text)

    def fallback_action(self, obs):
        return "drive:up,8"  # compass prior, same bias as the T1 baseline

    def _battle(self, env):
        # T1 is buttons-only: the model fights by picking buttons.
        action, _ = self.decide(env, self._task, self._obs)
        obs, outcome, _ = env.step(action)
        if obs["mode"] != "battle":
            self.battles_won += 1
            self._in_battle = False
        return obs, outcome, outcome["done"]

    def run(self, env, task, frame_budget):
        self._task = task
        self._obs = None
        return super().run(env, task, frame_budget)


# ── T2: skill actions + symbolic observation ─────────────────────────
class SkillLlmAgent(LlmAgent):
    POLICY_NAME = "llm_skills"
    TIER = "T2"

    def system_prompt(self, task):
        return (
            "You play Pokémon Red through a navigation API. Goal: "
            f"{_goal_text(task)}. Each turn I report map, position, mode "
            "and nearby entities. You answer with EXACTLY ONE action, one "
            "of: `move_to:X,Y` (walk to tile), `interact_with:<id>` (talk "
            "to / pick up a nearby entity), `interact` (A in place), "
            "`travel_to:MapName` (cross-map travel), `press:a`, "
            "`step_frames:N` (wait). No prose.")

    def describe(self, env, task, obs):
        pos = obs["position"]
        return (f"map={obs['map']['name']} x={pos['x']} y={pos['y']} "
                f"mode={obs['mode']}. {_nearby_text(env)}. "
                f"Goal: {_goal_text(task)}. Action?")

    def parse_action(self, text):
        return parse_t2_action(text)

    def fallback_action(self, obs):
        # Head for the north map edge — the same compass prior the
        # calibration T2 uses (routes in these tasks run north).
        pos = obs["position"]
        return f"move_to:{pos['x']},{max(1, pos['y'] - 4)}"


# ── T3: T2 + world model in the prompt ────────────────────────────────
class WorldModelLlmAgent(SkillLlmAgent):
    POLICY_NAME = "llm_world_model"
    TIER = "T3"

    def system_prompt(self, task):
        return (
            "You play Pokémon Red with a navigation API and a world model "
            "(the map graph). Goal: "
            f"{_goal_text(task)}. Each turn I report position, nearby "
            "entities and the route the world graph suggests. You answer "
            "with EXACTLY ONE action: `move_to:X,Y`, `interact_with:<id>`, "
            "`interact`, `travel_to:MapName`, `press:a`, `step_frames:N`. "
            "No prose.")

    def describe(self, env, task, obs):
        base = super().describe(env, task, obs)
        goal = task["goal"]
        if goal["type"] == "map" and obs["map"]["name"] != goal["id"]:
            try:
                route = env.client.route(obs["map"]["name"], goal["id"])
                legs = route.get("legs") or []
                if legs:
                    chain = " → ".join([obs["map"]["name"]]
                                       + [leg["to_map"] for leg in legs])
                    return base + f" World graph route: {chain}."
            except Exception:
                pass
        return base
