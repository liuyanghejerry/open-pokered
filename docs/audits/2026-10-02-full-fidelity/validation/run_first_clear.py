"""Run the existing fresh milestone driver with deterministic frame stepping.

No warp, give_*, flags, snapshot, or development resume is used.
Run from the repository root with Python 3.
"""
from pathlib import Path
import sys
import os
import time

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "scripts"))
import playthrough

# The existing two-command queue/step driver races the driven-only loop,
# which drains queued input before the next command arrives. Execute each
# input burst and its neutral tail in one synchronous timeline command.
def atomic_drive(self, buttons, frames=None):
    timeline = list(buttons)
    count = len(timeline) if frames is None else frames
    if count < len(timeline):
        raise ValueError("frame count cannot be shorter than input timeline")
    timeline.extend([None] * (count - len(timeline)))
    return self.cmd(cmd="press_timeline", buttons=timeline, advance=True)


playthrough.DebugClient.drive = atomic_drive
OriginalGame = playthrough.Game


class AuditGame(OriginalGame):
    def __init__(self, *args, **kwargs):
        kwargs.setdefault("speed", 0)
        kwargs.setdefault("seed", 42)
        if os.environ.get("AUDIT_BINARY"):
            kwargs.setdefault("binary", os.environ["AUDIT_BINARY"])
        super().__init__(*args, **kwargs)

    def st(self):
        state = super().st()
        now = time.monotonic()
        if now >= getattr(self, "_next_observation_log", 0):
            live = state.get("battle_live") or {}
            player, enemy = live.get("player", {}), live.get("enemy", {})
            print(
                f"[live] frame={state.get('frame')} screen={state.get('screen')} "
                f"map={state.get('map_name')} "
                f"pos=({state.get('player_x')},{state.get('player_y')}) "
                f"phase={state.get('battle_phase')} "
                f"HP={player.get('hp')}/{enemy.get('hp')}",
                flush=True,
            )
            self._next_observation_log = now + 60
        return state

    def battle_loop(self, prefer="fight", max_iters=1600):
        # A long, legitimate trainer fight can exceed the driver's 400
        # input iterations when the lead's stronger move has no PP left.
        # Continue real menu/text input instead of treating that budget
        # as an engine stall. No battle state or inventory is changed.
        return super().battle_loop(prefer=prefer, max_iters=max_iters)


playthrough.Game = AuditGame
if __name__ == "__main__":
    playthrough.main()
