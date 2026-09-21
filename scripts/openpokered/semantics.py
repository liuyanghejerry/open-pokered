"""Semantic judgments for the agent stack and the test suite.

Two places in this repo need *semantic* understanding — not computation,
not a lookup — and used to fake it with brittle string work:

- **Agent targeting.** `policies.LocalExplorer` resolved a goal like
  "talk to Oak" by testing `"oak" in npc_name.lower()` over a hardcoded
  hint table, so any paraphrase the hint table did not anticipate was
  invisible. Here the candidates come from `get_nearby` (code owns the
  enumeration) and a Choice question picks the one the goal refers to.
- **Dialogue assertions.** The content-regression and BDD layers assert
  with `needle in text`, which tolerates line-break reflow but breaks on
  any wording change. A Noul question asks whether the line *conveys* the
  claim, which is what the test actually means.

Design rule (from the TypeSafe skill): code keeps the enumeration, the
policy, and the action; the model supplies only the judgment. So candidates
are always gathered in code, thresholds live in code, and every entry
point here returns `None` rather than guessing when the model is
unavailable — callers fall back to their previous deterministic path.

Note on `None`: a failed call degrades, it never invents an answer. That
keeps a flaky network from silently changing a test verdict or an agent
decision.
"""
from .typesafe import (Choice, CredentialError, Noul, TypeSafeClient,
                       TypeSafeError, configured)

# A Noul in this band means "as likely yes as no". Callers that act on the
# answer should treat it as unresolved rather than picking a side.
UNRESOLVED_BAND = (0.4, 0.6)

# Entity id the model may return instead of a candidate. Kept out of the
# candidate namespace so a real id can never collide with it.
NO_MATCH = "none"


class Judgment:
    """A Noul result with the raw probability kept visible.

    `holds` is the threshold decision; `resolved` says whether the
    probability is far enough from 0.5 to act on. Both are properties of
    the same number, so callers that need a different policy can read
    `probability` directly instead of guessing at a band.
    """

    def __init__(self, probability, claim, source="", threshold=0.5):
        self.probability = probability
        self.claim = claim
        self.source = source
        self.threshold = threshold

    @property
    def holds(self):
        return self.probability >= self.threshold

    @property
    def resolved(self):
        lo, hi = UNRESOLVED_BAND
        return not (lo <= self.probability <= hi)

    def __repr__(self):
        return (f"Judgment({self.probability:.3f}, holds={self.holds}, "
                f"resolved={self.resolved})")


class Selection:
    """A Choice result: the winner plus the distribution behind it.

    `confidence` is how concentrated the distribution is — not whether
    the pick is right. `runner_up` and `margin` make a close call
    visible so the caller can decline to act on it.
    """

    def __init__(self, choice, probabilities, confidence, source=""):
        self.choice = choice
        self.probabilities = probabilities
        self.confidence = confidence
        self.source = source

    @property
    def runner_up(self):
        ranked = sorted(self.probabilities.items(), key=lambda kv: -kv[1])
        return ranked[1][0] if len(ranked) > 1 else None

    @property
    def margin(self):
        ranked = sorted(self.probabilities.values(), reverse=True)
        return ranked[0] - ranked[1] if len(ranked) > 1 else 1.0

    def __repr__(self):
        return f"Selection({self.choice!r}, margin={self.margin:.3f})"


def render_candidate(candidate):
    """One candidate as the model sees it — a nearby entity or a place.

    The description carries what a reader would use to tell candidates
    apart: what it is, what it is called, where it is, how far away. Code
    keeps the id; the model only ever picks from the ids it is given.
    """
    pos = candidate.get("position") or {}
    kind = candidate.get("kind") or "entity"
    name = candidate.get("name") or ""
    bits = [kind]
    if name:
        bits.append(f"named {name!r}")
    if candidate.get("note"):
        bits.append(candidate["note"])
    if pos:
        bits.append(f"at ({pos.get('x')},{pos.get('y')})")
    if candidate.get("distance") is not None:
        bits.append(f"{candidate['distance']} tiles away")
    return ", ".join(bits)


def _target_question(candidates, what):
    """The Choice over candidate ids, with NO_MATCH as an explicit way out.

    One definition for every caller, so the wording cannot drift between
    entities and places — and `NO_MATCH` stays a first-class answer rather
    than something the model has to express by picking a bad candidate.
    """
    return Choice(
        instructions=(
            f"Which single {what}, if any, does the task goal refer to? "
            f"Pick the {what} the goal names or describes. "
            f"Pick {NO_MATCH!r} if none matches, including when the goal's "
            f"target is not in the list."),
        criteria={**{c["id"]: render_candidate(c) for c in candidates},
                  NO_MATCH: f"no listed {what} matches the goal"})


def _action_question(actions):
    """The closed set of things the caller can actually execute."""
    return Choice(
        instructions=(
            "Which single action best advances the goal from the current "
            "observation?"),
        criteria=dict(actions))


class SemanticJudge:
    """Typed judgments, with call/token accounting for run reports.

    `client` may be injected (tests pass a stub). When it is absent the
    judge is disabled and every method returns `None`.
    """

    def __init__(self, client=None, model=None, enabled=None):
        self.client = client
        self.model = model
        self.enabled = bool(client) if enabled is None else bool(enabled)
        self.calls = 0
        self.input_tokens = 0
        self.output_tokens = 0
        self.errors = []

    @classmethod
    def from_env(cls, env=None, **kw):
        """Enabled only when a key is available; otherwise a no-op judge."""
        if not configured(env):
            return cls(client=None, enabled=False, **kw)
        return cls(client=TypeSafeClient.from_env(env), **kw)

    # ── request plumbing ────────────────────────────────────────────
    def _ask(self, state, questions):
        """One request, many independent questions.

        Questions in a single call run in parallel and cannot see one
        another's answers, so each must stand alone. Returns None on any
        failure — the caller falls back to its deterministic path.
        """
        if not self.enabled or self.client is None:
            return None
        try:
            result = self.client.system_one(
                state, questions, model=self.model)
        except (TypeSafeError, CredentialError) as e:
            self.errors.append(f"{type(e).__name__}: {e}")
            return None
        self.calls += 1
        self.input_tokens += result.input_tokens
        self.output_tokens += result.output_tokens
        return result.answers

    # ── targeting: entities and places ──────────────────────────────
    def _choose_from(self, state, candidates, what):
        """One Choice over `candidates`; None only when the call failed."""
        if not candidates:
            return None
        answers = self._ask(state,
                            {"target": _target_question(candidates, what)})
        if answers is None or "target" not in answers:
            return None
        answer = answers["target"]
        return Selection(answer.choice, answer.probabilities,
                         answer.confidence)

    def ask_for_entity(self, goal, candidates):
        """The raw Choice over `candidates`, `NO_MATCH` included.

        Returns None only when the call itself failed, so a caller can
        tell "the model rejected every candidate" from "the model never
        answered".
        """
        return self._choose_from(goal, candidates, "entity")

    def select_entity(self, goal, candidates):
        """The candidate id `goal` refers to, or None.

        `candidates` must be the complete set the goal could refer to —
        the model cannot choose an id that was never offered, so gather
        them with a radius that covers the map before asking.
        """
        selection = self.ask_for_entity(goal, candidates)
        if selection is None or selection.choice == NO_MATCH:
            return None
        return selection.choice

    def choose_objective(self, situation, candidates):
        """Which outstanding story objective is worth pursuing next.

        Unlike the other targeting judgments, the goal is not given — it
        is what this question answers. Code narrows to the objectives whose
        flag is still unset and what the script index knows about each; the
        judgment picks the thread.
        """
        if not candidates:
            return None
        answers = self._ask(situation, {"objective": Choice(
            instructions=(
                "Which single outstanding story objective is worth pursuing "
                "next? Pick the one that best advances the game from the "
                f"current situation. Pick {NO_MATCH!r} if none is pursuable "
                "right now."),
            criteria={**{c["id"]: render_candidate(c) for c in candidates},
                      NO_MATCH: "no listed objective is pursuable now"})})
        if answers is None or "objective" not in answers:
            return None
        choice = answers["objective"].choice
        return None if choice == NO_MATCH else choice

    def choose_destination(self, goal, candidates, location=None):
        """Which place, if any, does the goal point at? None for no match.

        `candidates` are the places code has already established are worth
        considering, and that narrowing is the point: a world graph here
        holds 222 maps and 1060 edges — too many to offer, and mostly
        unreachable from where the agent stands. Code walks the graph out
        to a hop budget and the judgment chooses within that set. A map
        outside the set cannot be chosen, so the budget *is* the
        candidate set, and widening it is how recall improves.
        """
        state = goal if location is None else {"goal": goal,
                                               "currently_at": location}
        selection = self._choose_from(state, candidates, "place")
        if selection is None or selection.choice == NO_MATCH:
            return None
        return selection.choice

    # ── action routing ──────────────────────────────────────────────
    def route_action(self, goal, actions, observation):
        """Pick one action from a closed set (function-calling shape).

        `actions` maps an action string (what the caller will execute) to
        a description of what it does. Only these are offered, so the
        result is always executable.
        """
        if not actions:
            return None
        answers = self._ask(
            {"goal": goal, "observation": observation},
            {"action": _action_question(actions)})
        if answers is None or "action" not in answers:
            return None
        answer = answers["action"]
        return Selection(answer.choice, answer.probabilities,
                         answer.confidence)

    def decide(self, goal, candidates, actions, observation, what="entity"):
        """Target and action for one agent step, asked together.

        These two judgments share the same state and neither needs the
        other's answer, so one request covers both — the skill's
        speculative fan-out: they run in parallel and the caller consumes
        whichever it needs. Returns `(target_selection, action_selection)`;
        either may be None.
        """
        questions = {}
        if candidates:
            questions["target"] = _target_question(candidates, what)
        if actions:
            questions["action"] = _action_question(actions)
        answers = self._ask({"goal": goal, "observation": observation,
                             "candidates": [
                                 {"id": c["id"], "description":
                                  render_candidate(c)} for c in candidates]},
                            questions)
        if answers is None:
            return None, None

        target = answers.get("target")
        action = answers.get("action")
        return (
            Selection(target.choice, target.probabilities, target.confidence,
                      source=goal) if target else None,
            Selection(action.choice, action.probabilities, action.confidence,
                      source=goal) if action else None,
        )

    # ── dialogue assertions ─────────────────────────────────────────
    def conveys(self, text, claim, context=None):
        """Does `text` convey `claim`, however it is worded?

        This is what a dialogue test means; substring matching is an
        approximation that breaks on reflow and rewording.

        `context` carries what a reader would know but the line does not
        say — who or where the speaker is. A claim that leans on such a
        fact ("came *here* with friends") is correctly judged unsupported
        without it, so pass the context rather than weakening the claim.
        """
        state = text if context is None else {"text": text, "context": context}
        answers = self._ask(state, {"conveys": Noul(
            instructions=f"Does the text convey this claim? Claim: {claim}",
            true="the text states or clearly implies the claim",
            false="the text does not say this")})
        if answers is None or "conveys" not in answers:
            return None
        return Judgment(answers["conveys"].noul, claim)

    def same_meaning(self, left, right):
        """Do two texts say the same thing? Used to separate a harmless
        reflow from a real wording or meaning change."""
        answers = self._ask({"left": left, "right": right}, {"same": Noul(
            instructions=(
                "Do the two texts convey the same meaning? Ignore "
                "differences in line breaks and whitespace; a change in "
                "wording that preserves the meaning still counts as the "
                "same."),
            true="same meaning",
            false="different meaning or a changed fact")})
        if answers is None or "same" not in answers:
            return None
        return Judgment(answers["same"].noul, "same meaning")
