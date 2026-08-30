#!/usr/bin/env python3
"""Minimal driver for the pokered debug-server JSON-line protocol.

Usage as a library:
    from debug_drive import DebugClient
    d = DebugClient(9000)
    d.cmd(cmd="press_sequence", buttons=["up"] * 40)
    d.cmd(cmd="step_frames", count=40)
    print(d.cmd(cmd="get_state")["data"])

Or run directly for a smoke test:
    python3 debug_drive.py [--port 9000]
"""
import json
import socket
import time


class DebugClient:
    def __init__(self, port=9000, host="127.0.0.1", connect_timeout=15.0):
        self.host, self.port = host, port
        self.sock = None
        self.f = None
        self._connect(connect_timeout)
        # Long synchronous commands (wait_until with a big frame budget)
        # legitimately take tens of seconds server-side; the connect
        # timeout must not apply to command round trips.
        self.sock.settimeout(120.0)
        self.f = self.sock.makefile("rw")

    def _connect(self, connect_timeout=15.0):
        deadline = time.time() + connect_timeout
        while True:
            try:
                self.sock = socket.create_connection(
                    (self.host, self.port), timeout=5)
                return
            except OSError:
                if time.time() > deadline:
                    raise
                time.sleep(0.5)

    def _reconnect(self):
        """Drop the (possibly wedged) connection and re-open one. The
        server reads requests one line at a time, so closing our side
        makes its reader hit EOF and accept the new connection fast."""
        try:
            if self.sock:
                self.sock.close()
        except OSError:
            pass
        self._connect(60)
        self.sock.settimeout(120.0)
        self.f = self.sock.makefile("rw")

    def cmd(self, **kw):
        """Send one JSON-line command, return the parsed response.
        Retries once on a transport stall (deadlocked round trip):
        commands are effectively idempotent for a closed-loop driver —
        a doubled queue entry or extra stepped frames self-correct."""
        line = json.dumps(kw) + "\n"
        for attempt in range(2):
            try:
                self.f.write(line)
                self.f.flush()
                resp = self.f.readline()
                if not resp:
                    raise OSError("connection closed by peer")
                return json.loads(resp)
            except OSError as e:
                if attempt:
                    raise
                print(f"[debug-drive] transport stall ({e!r}), "
                      f"reconnecting and retrying: {kw.get('cmd')}",
                      flush=True)
                self._reconnect()

    def close(self):
        self.f.close()
        self.sock.close()

    # ── Convenience helpers ─────────────────────────────────────────
    def press(self, button):
        return self.cmd(cmd="press", button=button)

    def press_sequence(self, buttons):
        return self.cmd(cmd="press_sequence", buttons=list(buttons))

    def step(self, count):
        """Synchronously advance `count` frames; returns when done."""
        return self.cmd(cmd="step_frames", count=count)

    def drive(self, buttons, frames=None):
        """Queue buttons then step exactly len(buttons) (or `frames`) frames."""
        self.press_sequence(buttons)
        return self.step(frames if frames is not None else len(buttons))

    def state(self):
        return self.cmd(cmd="get_state")["data"]

    def npcs(self):
        return self.cmd(cmd="get_npcs")["data"]


if __name__ == "__main__":
    import argparse

    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, default=9000)
    args = p.parse_args()

    d = DebugClient(args.port)
    st = d.state()
    print(
        "map={map_name} pos=({player_x},{player_y}) screen={screen} "
        "eff={active_script_effect}".format(**st)
    )
    print("npcs:", [(n["text_id"], n["x"], n["y"], n["visible"]) for n in d.npcs()])
    d.close()
