"""Observe Buffer2 copies after actual menu exits; never write emulator memory.

Requires PyBoy 2.8.1 or 2.9.0 and the normal Red build from pret/pokered
fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c. Output goes to stdout only.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path

from pyboy import PyBoy


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, required=True)
    parser.add_argument("--symbols", type=Path, required=True)
    parser.add_argument("--state", type=Path, required=True)
    args = parser.parse_args()
    rom, state = args.rom.read_bytes(), args.state.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == "5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b"
    assert hashlib.sha256(state).hexdigest() == "b12777818a990843546b623615caa67a05a811941dfcf4d078b46a564d6e09a4"
    symbols = {}
    for line in args.symbols.read_text().splitlines():
        parts = line.split()
        if len(parts) == 2 and ":" in parts[0]:
            symbols[parts[1]] = tuple(int(n, 16) for n in parts[0].split(":"))
    start = symbols["LoadScreenTilesFromBuffer2"][1]
    copy = symbols["CopyData"][1]
    assert rom[start] == 0xCD and rom[start + 7] == 0xC9
    assert rom[copy:copy + 3] == bytes([0x2A, 0x12, 0x13])

    for case, cursor in [("dex", 0), ("trainer", 3), ("bag", 2), ("option", 5)]:
        previous = None
        for repeat in [1, 2]:
            game = PyBoy(io.BytesIO(rom), window="null", sound_emulated=True)
            try:
                game.set_emulation_speed(0)
                game.load_state(io.BytesIO(state))
                get = lambda name: int(game.memory[symbols[name][1]])
                for button in ["a", "b", "start", "select", "up", "down", "left", "right"]:
                    game.button_release(button)
                game.tick(8)
                game.button_press("b")
                game.tick(2)
                game.button_release("b")
                game.tick(160)
                for _ in range(7):
                    if get("wCurrentMenuItem") == cursor:
                        break
                    game.button_press("down")
                    game.tick(2)
                    game.button_release("down")
                    game.tick(20)
                assert get("wCurrentMenuItem") == cursor
                game.button_press("a")
                game.tick(2)
                game.button_release("a")
                game.tick(240)

                observed, stores, interrupts = {}, [], []

                def active():
                    return bool(observed) and "cycles" not in observed

                def enter(_):
                    if not observed:
                        address = symbols["wTileMapBackup2"][1]
                        observed.update(start=game._cycles(), bg_before=get("hAutoBGTransferEnabled"),
                                        source=bytes(game.memory[address:address + 360]))

                def stored(_):
                    if active():
                        assert get("hAutoBGTransferEnabled") == 0
                        index = len(stores)
                        assert index < 360
                        assert game.memory[symbols["wTileMap"][1] + index] == observed["source"][index]
                        stores.append(game._cycles() - observed["start"])

                def interrupt(_):
                    if active():
                        interrupts.append(game._cycles())

                def returned(_):
                    if active():
                        address = symbols["wTileMap"][1]
                        assert bytes(game.memory[address:address + 360]) == observed["source"]
                        observed.update(cycles=game._cycles() + 16 - observed["start"],
                                        bg_after=get("hAutoBGTransferEnabled"))

                game.hook_register(0, start, enter, None)
                game.hook_register(0, start + 7, returned, None)
                game.hook_register(0, copy + 2, stored, None)
                game.hook_register(*symbols["VBlank"], interrupt, None)
                game.button_press("b")
                game.tick(2)
                game.button_release("b")
                game.tick(60)
                assert observed["cycles"] == 18_884 and not interrupts
                assert observed["bg_before"] == observed["bg_after"] == 1
                assert stores == [116 + 52 * index for index in range(360)]
                result = dict(case=case, cycles=observed["cycles"], stores=len(stores),
                              first_store=stores[0], last_store=stores[-1],
                              auto_bg_disabled_for_every_store=True, interrupts=len(interrupts))
                if previous is not None:
                    assert result == previous
                previous = result
                print(json.dumps(dict(result, repeat=repeat)), flush=True)
            finally:
                game.stop(save=False)


if __name__ == "__main__":
    main()
