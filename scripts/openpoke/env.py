"""OpenPokeEnv: a gym-style wrapper around the pokered agent debug API (M6).

Plain-class API (no required deps):
    env = OpenPokeEnv()
    obs = env.reset(task)
    obs, outcome, info = env.step("travel_to:ViridianCity")
    env.close()

`reset(task)` launches a fresh headless game with the task's seed and
initial state, applies the task's setup (flags/party/items), and returns
the first observation. `step(action)` executes ONE semantic action
("travel_to:X", "move_to:x,y", "interact", "interact_with:id",
"press:a", "skill:heal_pewter") and reports done/success via the task's
goal. NO reward logic inside: `compute_reward` is a caller hook; `info`
carries everything a reward needs (map visits, flag diffs, badges,
blackouts, invalid actions).

If `gymnasium` is importable, `make_gymnasium_env()` adapts this class to
the real `gymnasium.Env` protocol; without it the plain class is enough.
"""
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from debug_drive import DebugClient  # noqa: E402

from .client import AgentClient
from .tasks import goal_satisfied

ROOT = Path(__file__).resolve().parent.parent.parent
BIN = ROOT / "target" / "debug" / "pokered-app"


def _free_port():
    import socket
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


class OpenPokeEnv:
    def __init__(self, binary=None, launch_timeout=20.0, maps_dir=None):
        self.binary = Path(binary) if binary else BIN
        self.launch_timeout = launch_timeout
        # M7: an alternate maps tree (world variant). The desktop build
        # loads map data from the filesystem (no embedded-map-data), so
        # POKERED_MAPS_DIR redirects map.json/map.blk and --scripts-dir
        # redirects script.scene/script_config.json.
        self.maps_dir = Path(maps_dir) if maps_dir else None
        self.proc = None
        self.client = None
        self.task = None
        self.run_dir = None
        # run accounting (shared with metrics.py)
        self.env_steps = 0
        self.invalid_actions = 0
        self.battles = 0
        self.battles_won = 0
        self.blackouts = 0
        self.recovery_events = 0
        self._flags_baseline = {}
        self._maps_seen = set()

    # ── process management ──────────────────────────────────────────
    def _spawn(self, task):
        self.close()
        self.run_dir = Path(tempfile.mkdtemp(prefix="openpoke-"))
        port = _free_port()
        initial = task["initial_state"]
        cmd = [str(self.binary), "run", "--headless", "--debug-port", str(port),
               "--no-audio", "--seed", str(task["seed"]),
               "--save", str(self.run_dir / "run.sav")]
        if "snapshot" in initial:
            cmd += ["--snapshot", str(initial["snapshot"])]
        elif "save" in initial:
            cmd += ["--save", str(initial["save"])]
        if "warp" in initial:
            cmd += ["--skip-intro", "--warp", initial["warp"]]
        spawn_env = None
        if self.maps_dir is not None:
            cmd += ["--scripts-dir", str(self.maps_dir)]
            spawn_env = dict(os.environ, POKERED_MAPS_DIR=str(self.maps_dir))
        self.proc = subprocess.Popen(
            cmd, cwd=str(ROOT), env=spawn_env, stdout=subprocess.DEVNULL,
            stderr=open(self.run_dir / "game.log", "w"))
        self.client = AgentClient(port)
        # Wait for the game to accept commands (skip-intro boot takes a moment).
        deadline = time.time() + self.launch_timeout
        while True:
            try:
                self.client.observe(level=1)
                break
            except Exception:
                if time.time() > deadline:
                    raise RuntimeError("game did not come up in time")
                time.sleep(0.5)
        setup = task.get("setup", {})
        for flag, value in setup.get("flags", {}).items():
            self.client.set_flag(flag, value)
        for mon in setup.get("party", []):
            self.client.give_pokemon(mon["species"], mon["level"])
        for item in setup.get("items", []):
            self.client.give_item(item["item"], item.get("qty", 1))

    def close(self):
        if self.client is not None:
            try:
                self.client.close()
            except Exception:
                pass
            self.client = None
        if self.proc is not None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)
            self.proc = None

    # ── gym-style API ───────────────────────────────────────────────
    def reset(self, task):
        self._spawn(task)
        self.task = task
        self.env_steps = 0
        self.invalid_actions = 0
        self.battles = 0
        self.battles_won = 0
        self.blackouts = 0
        self.recovery_events = 0
        self._flags_baseline = self.client.flags()
        obs = self.client.observe()
        self._maps_seen.add(obs["map"]["name"])
        return obs

    def available_actions(self):
        mode = (self.task or {}).get("allowed_actions", "semantic")
        if mode == "controller":
            return [f"press:{b}" for b in ("a", "b", "up", "down", "left", "right", "start", "select")]
        nearby = self.client.nearby()
        actions = ["travel_to", "interact", "get_state"]
        actions += [f"interact_with:{e['id']}" for e in nearby.get("entities", [])
                    if e.get("interactable")]
        return actions

    def frame_count(self):
        return self.client.state()["frame_count"]

    def step(self, action):
        if self.task is None:
            raise RuntimeError("reset(task) first")
        self.env_steps += 1
        info = {"action": action, "invalid": False, "result": None}
        try:
            info["result"] = self._execute(action)
        except Exception as e:  # AgentError → invalid action; others propagate
            from .client import AgentError
            if isinstance(e, AgentError):
                self.invalid_actions += 1
                info["invalid"] = True
                info["result"] = {"error": str(e)}
            else:
                raise
        obs = self.client.observe()
        done, success = self._assess(info)
        info.update(self._reward_inputs(obs, success))
        return obs, {"done": done, "success": success}, info

    def _execute(self, action):
        verb, _, arg = action.partition(":")
        if verb == "travel_to":
            out = self.client.travel_to(arg)
            self._count_travel_outcome(out)
            return out
        if verb == "move_to":
            x, y = (int(v) for v in arg.split(","))
            out = self.client.move_to(x, y)
            self._count_nav_outcome(out)
            return out
        if verb == "interact":
            return self.client.interact()
        if verb == "interact_with":
            out = self.client.interact_with(arg)
            self._count_nav_outcome(out.get("navigation") or {})
            return out
        if verb == "press":
            self.client.press(arg)
            self.client.step(1)
            return {"pressed": arg}
        if verb == "step_frames":
            self.client.step(int(arg or 1))
            return {"stepped": int(arg or 1)}
        raise ValueError(f"unknown action verb {verb!r}")

    def _count_nav_outcome(self, out):
        result = out.get("result")
        if result == "entered_battle":
            self.battles += 1
            self.recovery_events += 1
        elif result == "entered_dialogue":
            self.recovery_events += 1

    def _count_travel_outcome(self, out):
        battles = out.get("battles", 0)
        self.battles += battles
        self.battles_won += battles
        if out.get("result") == "map_changed" or battles:
            self.recovery_events += battles
        if out.get("result") == "blackout":
            self.blackouts += 1

    def _assess(self, info):
        if goal_satisfied(self.task["goal"], self):
            return True, True
        if self.env_steps >= self.task["max_steps"]:
            return True, False
        obs = self.client.observe()
        if obs["mode"] == "battle":
            return False, False
        return False, False

    def _reward_inputs(self, obs, success):
        flags_now = self.client.flags()
        changed = sorted(k for k, v in flags_now.items()
                         if v and not self._flags_baseline.get(k))
        new_maps = sorted({obs["map"]["name"]} - self._maps_seen)
        self._maps_seen.add(obs["map"]["name"])
        self._flags_baseline = flags_now
        return {
            "success": success,
            "new_map_visited": new_maps,
            "flags_gained": changed,
            "badges": obs.get("badges", {}).get("count", 0),
            "env_steps": self.env_steps,
            "invalid_actions": self.invalid_actions,
            "battles": self.battles,
            "blackouts": self.blackouts,
        }

    # Reward hook: experiment-side, never called internally unless the
    # caller wires it (kept out of the class on purpose — see module docstring).
    compute_reward = None


def make_gymnasium_env(**kwargs):
    """Adapt OpenPokeEnv to the `gymnasium` protocol when it's installed."""
    import gymnasium as gym
    from gymnasium import spaces

    class GymEnv(gym.Env):
        metadata = {"render_modes": []}

        def __init__(self, task, **env_kwargs):
            super().__init__()
            self.impl = OpenPokeEnv(**env_kwargs)
            self.task = task
            self.action_space = spaces.Text(max_length=64)
            self.observation_space = spaces.Dict({})

        def reset(self, seed=None, options=None):
            task = dict(self.task)
            if seed is not None:
                task["seed"] = seed
            obs = self.impl.reset(task)
            return obs, {}

        def step(self, action):
            obs, outcome, info = self.impl.step(action)
            reward = 0.0
            if self.impl.compute_reward is not None:
                reward = self.impl.compute_reward(obs, info)
            return obs, reward, outcome["done"], False, info

        def close(self):
            self.impl.close()

    return GymEnv
