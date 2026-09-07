#!/usr/bin/env python3
"""Fresh m01–m10 with read-only battle diagnostics; optional isolated binary path."""
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import playthrough
if len(sys.argv) > 1:
    playthrough.BIN = Path(sys.argv.pop(1)).resolve()
original = playthrough.Game.battle_loop

def observed(self, *args, **kwargs):
    state = self.st()
    print("[battle-start]", state.get("map_name"), state.get("party"),
          state.get("battle_phase"), flush=True)
    result = original(self, *args, **kwargs)
    state = self.st()
    print("[battle-end]", state.get("map_name"), state.get("party"),
          state.get("battle_phase"), flush=True)
    return result

playthrough.Game.battle_loop = observed
playthrough.main()
