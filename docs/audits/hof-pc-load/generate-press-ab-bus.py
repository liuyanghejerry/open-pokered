#!/usr/bin/env python3
"""Independent finite PressAB instruction/access fixtures from the pinned ROM.

Controlled unit inputs; no Game, rendering, native tick or hardware emulation.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

from source_apu_access_oracle import run_with_accesses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("rom", "symbols", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location(
        "press_bus_oracle", Path(__file__).with_name("generate-timer-instructions.py"))
    oracle = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(oracle)
    rom = args.rom.read_bytes()
    assert hashlib.sha256(rom).hexdigest() == oracle.ROM_SHA
    symbols = {}
    for line in args.symbols.read_text().splitlines():
        parts = line.split()
        if len(parts) > 1 and ":" in parts[0] and not parts[0].startswith(";"):
            bank, address = parts[0].split(":")
            symbols[parts[1]] = (int(bank, 16), int(address, 16))
    directory = Path(__file__).resolve().parents[3] / "crates/pokered-audio/src/source_clock"
    previous = json.loads((directory / "press_ab_instructions_golden.json").read_text())["cases"]
    cases = []
    for old in previous:
        item, data = old["input"], old["data"]
        bank = item["bank"]
        prefix = "Audio1" if bank == 2 else "Audio3"
        suffix = "1" if bank == 2 else "3"
        _, voice = symbols["SFX_Press_AB_" + suffix + "_Ch5"]
        offset = bank * 0x4000 + voice - 0x4000
        assert list(rom[offset:offset + 19]) == data
        assert symbols[prefix + "_PlayNextNote"][0] == bank
        for nr51 in (0, 1, 0x44, 0xa5, 255):
            base, channel = 0xd100, 4
            memory = {base + index: value for index, value in enumerate(data)}
            for name, value in (
                ("wChannelSoundIDs", 144), ("wChannelFlags1", item["flags1"]),
                ("wChannelFlags2", item["flags2"]), ("wChannelNoteSpeeds", 1),
                ("wChannelNoteDelayCountersFractionalPart", item["fraction"]),
                ("wChannelDutyCycles", item["duty"]),
                ("wChannelVibratoDelayCounterReloadValues", item["vibrato_reload"]),
                ("wChannelNoteDelayCounters", 1),
            ):
                memory[symbols[name][1] + channel] = value
            pointer = symbols["wChannelCommandPointers"][1] + 2 * channel
            memory[pointer] = (base + item["ptr"]) & 255
            memory[pointer + 1] = (base + item["ptr"]) >> 8
            tempo = symbols["wSfxTempo"][1]
            memory[tempo], memory[tempo + 1], memory[0xff25] = 1, 0, nr51
            trace, _, final, events = run_with_accesses(
                oracle, rom, bank, dict(mute=0, sound_ids=[0] * 8, effects=[None] * 8),
                entry=symbols[prefix + "_PlayNextNote"][1], registers={"C": channel},
                memory=memory)
            assert trace == old["trace"]
            accesses = [dict(instruction=index, pc=pc, kind=kind.lower(), address=address,
                             value=value, at_tcycle=offset)
                        for index, pc, kind, address, value, offset in events]
            counts = tuple(sum(event[2] == kind for event in events) for kind in ("Read", "Write"))
            assert counts == ((0, 0) if item["ptr"] == 18 else (1, 6))
            cases.append(dict(input=item, data=data, nr51=nr51, trace=trace, accesses=accesses,
                              final_apu={str(address): value for address, value in final.items()
                                         if 0xff10 <= address <= 0xff3f}, result=old["result"]))
    result = dict(scope=__doc__, rom_sha256=oracle.ROM_SHA,
                  pret_revision="fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c", cases=cases)
    args.output.write_text(json.dumps(result, separators=(",", ":")) + "\n")
    print(len(cases), "PressAB cases;", sum(len(c["accesses"]) for c in cases), "actual APU accesses")


if __name__ == "__main__":
    main()
