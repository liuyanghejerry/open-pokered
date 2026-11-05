"""Validate the compact paired records without a ROM or an emulator."""
import gzip
import json
from pathlib import Path

root = Path(__file__).resolve().parent


def read(name):
    return json.loads(gzip.decompress((root / name).read_bytes()))


def pixels(data, roi, native=False):
    x, y, width, height = roi
    if native:
        # FrameBuffer::packed uses row-major eight-pixel GB bitplanes, not four
        # consecutive color indices per byte like the reference records.
        result = []
        for row in range(y, y + height):
            for col in range(x, x + width):
                offset = row * 40 + col // 8 * 2
                shift = 7 - col % 8
                result.append(((data[offset] >> shift) & 1)
                              | (((data[offset + 1] >> shift) & 1) << 1))
        return result
    return [
        (data[(row * 160 + col) // 4] >> (6 - 2 * (col % 4))) & 3
        for row in range(y, y + height)
        for col in range(x, x + width)
    ]


def without_grass(value):
    if isinstance(value, dict):
        return {k: without_grass(v) for k, v in value.items()
                if k not in ("pixels", "grass_priority")}
    if isinstance(value, list):
        return [without_grass(v) for v in value]
    return value


verification = json.loads((root / "verification.json").read_text())
for case in verification["windows"]:
    name, count, roi = case["case"], case["frames"], case["roi"]
    original = read(f"original-{name}-1.json.gz")["frames"]
    assert original == read(f"original-{name}-2.json.gz")["frames"]
    versions = {}
    for version in ("before", "after"):
        rows = read(f"native-{version}-{name}-1.json.gz")
        assert rows == read(f"native-{version}-{name}-2.json.gz")
        assert len(rows) == 121
        assert all(r["frame_count"] == rows[0]["frame_count"] + i
                   for i, r in enumerate(rows))
        differences = [sum(a != b for a, b in zip(
            pixels(bytes.fromhex(o["pixels"]), roi), pixels(c["pixels"], roi, native=True)))
            for o, c in zip(original[:count], rows[:count])]
        assert sum(n != 0 for n in differences) == case[f"{version}_different_frames"]
        if version == "before":
            assert max(differences) == case["before_max_wrong_pixels"]
        versions[version] = rows
    assert without_grass(versions["before"]) == without_grass(versions["after"])
    slot = int(name != "stand")
    prefix = f"wSprite0{slot + 1}StateData"
    for original_row, current in zip(original[:count], versions["after"][:count]):
        image = current["presentation"]["images"][slot]
        assert current["npc"]["walk_counter"] == original_row[prefix + "2WalkAnimationCounter"]
        assert image["phase"] == original_row[prefix + "1ImageIndex"] % 4
        assert image["grass_priority"] == bool(original_row[prefix + "2GrassPriority"])
        assert (current["player"]["x"], current["player"]["y"]) == (
            original_row["wXCoord"], original_row["wYCoord"])
    print(name, count, "frames: original pixels and state match; repeats identical")

for version in ("before", "after"):
    first, second = [read(f"gba-{version}-{n}.json.gz") for n in (1, 2)]
    assert first["status"] == second["status"] == "recorded"
    assert first["commands"] == second["commands"]
    assert first["frames"] == second["frames"]
    assert len(first["frames"]) == 1000
    assert all(row["physical_frame"] == first["origin"] + i + 1
               for i, row in enumerate(first["frames"]))
    for key in ("before_walk", "after_walk", "after_reopen"):
        assert first[key] == second[key]
    print("GBA", version, "1,000 consecutive physical frames: repeats identical")
