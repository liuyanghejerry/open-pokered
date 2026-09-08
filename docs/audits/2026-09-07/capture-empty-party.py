#!/usr/bin/env python3
"""Capture the new-game four-step poison regression using an isolated binary."""
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
import playthrough as pt
pt.BIN = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
g = pt.Game()
try:
    pt.m01_boot(g)
    pt.m02_oak_speech(g)
    g.d.drive(['right'] * 8, frames=12)
    g.d.drive(['up'] * 24, frames=124)
    state = g.st()
    print({k: state[k] for k in ['map_name', 'player_x', 'player_y', 'party_count']})
    result = g.d.cmd(cmd='capture_frame', path=str(out))
    assert result['ok'], result
finally:
    g.close()
