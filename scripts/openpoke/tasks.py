"""Task-spec loading and validation for openpoke experiments (M6).

A task JSON (see `scripts/openpoke/tasks/`) looks like:

    {
      "id": "reach-pewter-city",
      "name": "Reach Pewter City",
      "tier": 1,
      "initial_state": {"warp": "PalletTown,10,6"},
      "seed": 42,
      "setup": {
        "flags": {"EVENT_GOT_STARTER": true},
        "party": [{"species": "Mewtwo", "level": 100}],
        "items": [{"item": "Potion", "qty": 3}]
      },
      "goal": {"type": "map", "id": "PewterCity"},
      "allowed_actions": "semantic",
      "max_steps": 40
    }

Goal types: `flag` (EVENT_* set), `map` (player stands on map), `item`
(bag contains item), `party_count` (party has >= min), `battle_won`
(a wild/trainer battle was won this run and the player is back in the
overworld standing). All types the oracle can plan against.
"""
import json
from pathlib import Path

TASKS_DIR = Path(__file__).resolve().parent / "tasks"

GOAL_TYPES = {"flag", "map", "item", "party_count", "battle_won"}
ALLOWED_ACTIONS = {"semantic", "controller"}
REQUIRED_FIELDS = {"id", "name", "initial_state", "seed", "goal", "max_steps"}


class TaskSpecError(ValueError):
    pass


def load_task(path):
    """Load and validate one task spec file."""
    path = Path(path)
    with open(path) as f:
        spec = json.load(f)
    validate_task(spec, source=str(path))
    return spec


def load_tasks_dir(directory=None):
    """Load every task spec in the tasks directory, sorted by tier+id."""
    directory = Path(directory) if directory else TASKS_DIR
    tasks = [load_task(p) for p in sorted(directory.glob("*.json"))]
    return sorted(tasks, key=lambda t: (t.get("tier", 99), t["id"]))


def validate_task(spec, source="<spec>"):
    missing = REQUIRED_FIELDS - spec.keys()
    if missing:
        raise TaskSpecError(f"{source}: missing fields {sorted(missing)}")
    if not isinstance(spec["id"], str) or not spec["id"]:
        raise TaskSpecError(f"{source}: id must be a non-empty string")
    if not isinstance(spec["seed"], int):
        raise TaskSpecError(f"{source}: seed must be an integer")
    if not isinstance(spec["max_steps"], int) or spec["max_steps"] <= 0:
        raise TaskSpecError(f"{source}: max_steps must be a positive integer")

    goal = spec["goal"]
    gtype = goal.get("type")
    if gtype not in GOAL_TYPES:
        raise TaskSpecError(f"{source}: unknown goal type {gtype!r}")
    if gtype in {"flag", "map", "item"} and not goal.get("id"):
        raise TaskSpecError(f"{source}: goal type {gtype} requires an id")
    if gtype == "party_count" and not isinstance(goal.get("min"), int):
        raise TaskSpecError(f"{source}: goal type party_count requires min")
    if spec.get("allowed_actions", "semantic") not in ALLOWED_ACTIONS:
        raise TaskSpecError(
            f"{source}: allowed_actions must be one of {sorted(ALLOWED_ACTIONS)}")

    initial = spec["initial_state"]
    kinds = [k for k in ("warp", "snapshot", "save") if k in initial]
    if len(kinds) != 1:
        raise TaskSpecError(
            f"{source}: initial_state needs exactly one of warp/snapshot/save")
    if "warp" in initial and not isinstance(initial["warp"], str):
        raise TaskSpecError(f"{source}: initial_state.warp must be a string")

    setup = spec.get("setup", {})
    for flag, value in setup.get("flags", {}).items():
        if not flag.startswith("EVENT_"):
            raise TaskSpecError(f"{source}: setup flag {flag!r} must start with EVENT_")
        if not isinstance(value, bool):
            raise TaskSpecError(f"{source}: setup flag {flag} value must be bool")
    for mon in setup.get("party", []):
        if not mon.get("species") or not isinstance(mon.get("level"), int):
            raise TaskSpecError(f"{source}: setup party entries need species+level")
    for item in setup.get("items", []):
        if not item.get("item") or not isinstance(item.get("qty", 1), int):
            raise TaskSpecError(f"{source}: setup items need item+qty")
    return True


def goal_satisfied(goal, env):
    """Check the goal against the live game via the env's client."""
    gtype = goal["type"]
    if gtype == "flag":
        return bool(env.client.flags().get(goal["id"]))
    if gtype == "map":
        obs = env.client.observe(level=1)
        return obs["map"]["name"] == goal["id"]
    if gtype == "item":
        want = goal["id"].replace("_", "").lower()
        return any(
            entry["item"].replace("_", "").lower() == want
            for entry in env.client.bag()
        )
    if gtype == "party_count":
        return len(env.client.party()) >= goal["min"]
    if gtype == "battle_won":
        return env.battles_won > 0 and env.client.observe()["mode"] == "overworld"
    raise TaskSpecError(f"unknown goal type {gtype!r}")
