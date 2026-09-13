"""Per-run metrics for openpoke experiments (M6).

One `RunMetrics` per (task, seed) attempt; serialized as one JSON line
into `target/agent/runs/<task-id>.jsonl`. Counters are adapter-side so
the same record works for the rule-based oracle (model calls = 0) and
future LLM agents.
"""
import json
import time
from dataclasses import dataclass, field
from pathlib import Path

RUNS_DIR = Path(__file__).resolve().parent.parent.parent / "target" / "agent" / "runs"


@dataclass
class RunMetrics:
    task_id: str
    seed: int
    success: bool = False
    failure_reason: str = ""
    env_steps: int = 0
    wall_clock_s: float = 0.0
    frames_start: int = 0
    frames_end: int = 0
    invalid_actions: int = 0
    battles: int = 0
    battles_won: int = 0
    blackouts: int = 0
    # Navigation interruptions the run recovered from (battle/dialogue
    # handled by the executor, travel continued).
    recovery_events: int = 0
    # LLM-agent accounting (0 for the rule-based oracle).
    model_calls: int = 0
    model_tokens_prompt: int = 0
    model_tokens_completion: int = 0
    extra: dict = field(default_factory=dict)

    _t0: float = field(default=0.0, repr=False)

    def start(self, frames_start):
        self._t0 = time.time()
        self.frames_start = frames_start

    def finish(self, frames_end, success, reason=""):
        self.frames_end = frames_end
        self.wall_clock_s = round(time.time() - self._t0, 3)
        self.success = success
        self.failure_reason = reason

    @property
    def frames_elapsed(self):
        return self.frames_end - self.frames_start

    def to_dict(self):
        return {
            "task_id": self.task_id,
            "seed": self.seed,
            "success": self.success,
            "failure_reason": self.failure_reason,
            "env_steps": self.env_steps,
            "wall_clock_s": self.wall_clock_s,
            "frames_start": self.frames_start,
            "frames_end": self.frames_end,
            "frames_elapsed": self.frames_elapsed,
            "invalid_actions": self.invalid_actions,
            "battles": self.battles,
            "battles_won": self.battles_won,
            "blackouts": self.blackouts,
            "recovery_events": self.recovery_events,
            "model_calls": self.model_calls,
            "model_tokens_prompt": self.model_tokens_prompt,
            "model_tokens_completion": self.model_tokens_completion,
            "extra": self.extra,
        }

    def write(self, directory=None):
        directory = Path(directory) if directory else RUNS_DIR
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / f"{self.task_id}.jsonl"
        with open(path, "a") as f:
            f.write(json.dumps(self.to_dict()) + "\n")
        return path

    @staticmethod
    def read_runs(path):
        with open(path) as f:
            return [json.loads(line) for line in f if line.strip()]
