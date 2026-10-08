"""Drive the production debug handlers through an ignored app test.

Usage: python3 scripts/fidelity_stdio.py APP_TEST_BINARY --until m49
Build the test binary with cargo test -p pokered-app --lib --features
 debug-server --no-run. This transport needs no TCP port.
"""
import json
import subprocess
import tempfile
from pathlib import Path


def install(pt, debug, binary):
    pt.BIN = Path(binary)

    class PipeClient(debug.DebugClient):
        def __init__(self, proc):
            self.proc = proc

        def cmd(self, **kwargs):
            self.proc.stdin.write(json.dumps(kwargs) + "\n")
            self.proc.stdin.flush()
            while True:
                line = self.proc.stdout.readline()
                if not line:
                    raise RuntimeError("app test transport exited")
                # The test harness prefixes its first line with the test name.
                if "{" in line:
                    response = json.loads(line[line.index("{"):])
                    assert response["ok"], response
                    return response

        def close(self):
            self.proc.stdin.close()

        def drive(self, buttons, frames=None):
            buttons = list(buttons)
            count = len(buttons) if frames is None else frames
            assert count >= len(buttons), (count, len(buttons))
            return self.cmd(cmd="press_timeline", buttons=buttons + [None] * count,
                            advance=True)

    def initialize(self, port=None, save_path=None, record_dir=None,
                   record_video=None, snapshot=None, binary=None, seed=0, speed=0):
        if record_dir or record_video:
            raise ValueError("recording requires the native CLI transport")
        self.binary = Path(binary) if binary else pt.BIN
        self.seed, self.speed = seed, speed
        self.run_dir = Path(tempfile.mkdtemp(prefix="pokered-pipe-"))
        self.log = (self.run_dir / "game.log").open("w")
        self.persistent = save_path is not None
        self.save_path = Path(save_path) if save_path else self.run_dir / "play.sav"
        self.proc = subprocess.Popen(
            [str(self.binary), "--exact", "game::fidelity_stdio::driver",
             "--ignored", "--nocapture", "--test-threads=1"],
            cwd=str(pt.ROOT), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.log, text=True, bufsize=1,
        )
        self.d = PipeClient(self.proc)
        args = {"cmd": "initialize_fixture", "save": str(self.save_path),
                "seed": 0 if seed is None else seed}
        if snapshot:
            args["snapshot"] = str(snapshot)
        try:
            self.d.cmd(**args)
        except BaseException:
            self.close()
            raise
        self.frame0 = None
        self.last_map = "PalletTown"
        self.observed_npcs = {}

    pt.Game.__init__ = initialize


if __name__ == "__main__":
    import sys
    import playthrough as pt
    import debug_drive

    binary = sys.argv.pop(1)
    install(pt, debug_drive, binary)
    pt.main()
