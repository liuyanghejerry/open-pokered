#!/usr/bin/env python3
"""Frame-by-frame differential audit for all 165 Generation I move animations.

The reference side uses a pinned DEBUG ROM only to prepare one real battle,
loads that state into a pinned retail Red ROM in PyBoy, and replaces
wAnimationID at MoveAnimation entry. The current side invokes pokered-app's
isolated production renderer capture. Both captures exclude
PlayApplyingAttackAnimation so the comparison covers the move's own animation
command stream only.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import re
import shutil
import subprocess
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

import numpy as np
from PIL import Image


PINNED_ROM_SHA1 = "ea9bcae617fdf159b045185467ae58b2e4a48b9a"
PINNED_SETUP_ROM_SHA1 = "5b1456177671b79b263c614ea0e7cc9ac542e9c4"
PINNED_SYMBOLS_SHA1 = "03783c86a42588bd77f73bd7814cf8d70e590118"
PINNED_SOURCE_COMMIT = "fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c"
ANIM_BASE_TILE_ID = 0x31
MOVE_COUNT = 165


def sha1_file(path: Path) -> str:
    digest = hashlib.sha1()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_pinned(path: Path, expected: str, label: str) -> None:
    actual = sha1_file(path)
    if actual != expected:
        raise RuntimeError(f"{label} SHA-1 mismatch: expected {expected}, got {actual}")


def require_empty_output(path: Path) -> None:
    if path.exists() and any(path.iterdir()):
        raise RuntimeError(f"output directory must be empty: {path}")
    path.mkdir(parents=True, exist_ok=True)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def parse_symbols(path: Path) -> dict[str, tuple[int, int]]:
    symbols: dict[str, tuple[int, int]] = {}
    pattern = re.compile(r"^([0-9a-fA-F]{2}):([0-9a-fA-F]{4})\s+(\S+)$")
    for line in path.read_text().splitlines():
        match = pattern.match(line)
        if match:
            symbols[match.group(3)] = (int(match.group(1), 16), int(match.group(2), 16))
    required = {
        "MoveAnimation",
        "MoveAnimation.animationFinished",
        "PlayAnimation",
        "wAnimationID",
        "wAnimationType",
        "wBattleMonSpeed",
        "wEnemyMonSpeed",
        "wIsInBattle",
        "wOptions",
        "hWhoseTurn",
    }
    missing = sorted(required - symbols.keys())
    if missing:
        raise RuntimeError(f"reference symbol file lacks: {missing}")
    return symbols


def parse_move_names(source: Path) -> list[str]:
    constants = source / "constants" / "move_constants.asm"
    names: list[str] = []
    pattern = re.compile(r"^\s*const\s+([A-Z0-9_]+)\s*(?:;.*)?$")
    for line in constants.read_text().splitlines():
        match = pattern.match(line)
        if not match:
            continue
        name = match.group(1)
        if name == "NO_MOVE":
            continue
        names.append(name)
        if name == "STRUGGLE":
            break
    if len(names) != MOVE_COUNT:
        raise RuntimeError(f"expected {MOVE_COUNT} move constants, found {len(names)}")
    return names


def parse_id_set(value: str, maximum: int) -> list[int]:
    if value == "all":
        return list(range(1, maximum + 1))
    result: set[int] = set()
    for part in value.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            start_text, end_text = part.split("-", 1)
            start, end = int(start_text), int(end_text)
            result.update(range(start, end + 1))
        else:
            result.add(int(part))
    if not result or min(result) < 1 or max(result) > maximum:
        raise ValueError(f"move ids must be within 1..={maximum}")
    return sorted(result)


def rle(values: Iterable[Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for value in values:
        if result and result[-1]["value"] == value:
            result[-1]["count"] += 1
        else:
            result.append({"count": 1, "value": value})
    return result


def pixel_observation(frame: np.ndarray, baseline: np.ndarray) -> dict[str, Any]:
    rgb = frame[:, :, :3]
    baseline_rgb = baseline[:, :, :3]
    changed = np.any(rgb != baseline_rgb, axis=2)
    packed = np.packbits(changed, axis=None).tobytes()
    ys, xs = np.nonzero(changed)
    bbox = None if len(xs) == 0 else [int(xs.min()), int(ys.min()), int(xs.max() + 1), int(ys.max() + 1)]
    return {
        "delta_crc32": f"{zlib.crc32(packed):08x}",
        "delta_adler32": f"{zlib.adler32(packed):08x}",
        "changed_pixels": int(changed.sum()),
        "bbox": bbox,
    }


def trace_digest(pixels: list[dict[str, Any]], oam: list[Any]) -> str:
    payload = json.dumps(
        {"pixels": pixels, "oam": oam},
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return hashlib.sha1(payload).hexdigest()


def summarize_capture(
    pixel_trace: list[dict[str, Any]],
    rendered_oam: list[list[dict[str, int]]],
    *,
    source_oam: list[list[dict[str, int]]] | None = None,
    resolved_animation_id: int | None = None,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "duration_frames": len(pixel_trace),
        "trace_sha1": trace_digest(pixel_trace, rendered_oam),
        "pixel_delta_rle": rle(pixel_trace),
        "rendered_oam_rle": rle(rendered_oam),
        "max_rendered_objects": max((len(entries) for entries in rendered_oam), default=0),
    }
    if source_oam is not None:
        result["source_oam_rle"] = rle(source_oam)
        result["max_source_objects"] = max((len(entries) for entries in source_oam), default=0)
    if resolved_animation_id is not None:
        result["resolved_animation_id"] = resolved_animation_id
    return result


def expanded_rle(trace: list[dict[str, Any]]) -> list[Any]:
    result: list[Any] = []
    for item in trace:
        result.extend([item["value"]] * item["count"])
    return result


def compressed_values(trace: list[dict[str, Any]]) -> list[Any]:
    return [item["value"] for item in trace]


def visible_keyframes(trace: list[dict[str, Any]]) -> list[Any]:
    return [value for value in compressed_values(trace) if value]


def subtract_hardware_offsets(trace: list[Any]) -> list[Any]:
    return [
        [
            {
                **entry,
                "x": entry["x"] - 8,
                "y": entry["y"] - 16,
            }
            for entry in frame
        ]
        for frame in trace
    ]


@dataclass
class HookContext:
    pyboy: Any
    symbols: dict[str, tuple[int, int]]
    requested_id: int = 1
    whose_turn: int = 0
    started: bool = False
    finished: bool = False
    resolved_id: int | None = None

    def address(self, name: str) -> int:
        return self.symbols[name][1]


class ReferenceRunner:
    def __init__(self, rom: Path, setup_rom: Path, symbols: Path, max_frames: int):
        from pyboy import PyBoy

        self._temp = tempfile.TemporaryDirectory(prefix="move-anim-reference-")
        self.rom = Path(self._temp.name) / "reference.gbc"
        setup_copy = Path(self._temp.name) / "setup.gbc"
        shutil.copyfile(rom, self.rom)
        shutil.copyfile(setup_rom, setup_copy)
        self.symbols = parse_symbols(symbols)
        self.max_frames = max_frames
        # The DEBUG build is used only to create a deterministic live-battle
        # state. All observed animation frames execute in the pinned retail
        # Red ROM loaded below.
        self.pyboy = PyBoy(str(setup_copy), window="null", sound_emulated=False)
        self.stable_state = self._prepare_battle_menu()
        self.pyboy.stop(save=False)
        self.pyboy = PyBoy(str(self.rom), window="null", sound_emulated=False)
        self.pyboy.load_state(io.BytesIO(self.stable_state))
        if self.pyboy.memory[self._addr("wIsInBattle")] == 0:
            raise RuntimeError("retail reference ROM rejected the prepared battle state")
        self.context = HookContext(self.pyboy, self.symbols)
        self._register_hooks()

    @staticmethod
    def tap(pyboy: Any, button: str, frames: int, hold: int = 2) -> None:
        pyboy.button_press(button)
        pyboy.tick(hold)
        pyboy.button_release(button)
        pyboy.tick(frames - hold)

    @staticmethod
    def save_state(pyboy: Any) -> bytes:
        state = io.BytesIO()
        pyboy.save_state(state)
        return state.getvalue()

    def _addr(self, name: str) -> int:
        return self.symbols[name][1]

    def _prepare_battle_menu(self) -> bytes:
        pyboy = self.pyboy
        pyboy.tick(500)
        self.tap(pyboy, "start", 120)
        self.tap(pyboy, "start", 120)
        for _ in range(8):
            self.tap(pyboy, "select", 60)
        self.tap(pyboy, "a", 180)
        self.tap(pyboy, "b", 60)
        self.tap(pyboy, "b", 1400)
        self.tap(pyboy, "a", 200)
        self.tap(pyboy, "a", 200)
        if pyboy.memory[self._addr("wIsInBattle")] == 0:
            raise RuntimeError("reference setup did not reach a battle menu")
        return self.save_state(pyboy)

    def _register_hooks(self) -> None:
        context = self.context

        def at_move_animation(ctx: HookContext) -> None:
            memory = ctx.pyboy.memory
            if memory[ctx.address("hWhoseTurn")] != ctx.whose_turn or ctx.started:
                return
            memory[ctx.address("wAnimationID")] = ctx.requested_id
            memory[ctx.address("wAnimationType")] = 0
            ctx.started = True

        def at_play_animation(ctx: HookContext) -> None:
            if ctx.started and not ctx.finished:
                ctx.resolved_id = ctx.pyboy.memory[ctx.address("wAnimationID")]

        def at_animation_finished(ctx: HookContext) -> None:
            memory = ctx.pyboy.memory
            if ctx.started and memory[ctx.address("hWhoseTurn")] == ctx.whose_turn:
                ctx.finished = True

        for name, callback in (
            ("MoveAnimation", at_move_animation),
            ("PlayAnimation", at_play_animation),
            ("MoveAnimation.animationFinished", at_animation_finished),
        ):
            bank, address = self.symbols[name]
            self.pyboy.hook_register(bank, address, callback, context)

    def _visible_oam(self) -> list[dict[str, int]]:
        entries: list[dict[str, int]] = []
        for index in range(40):
            sprite = self.pyboy.get_sprite(index)
            if not sprite.on_screen:
                continue
            attributes = (
                (int(sprite.attr_obj_bg_priority) << 7)
                | (int(sprite.attr_y_flip) << 6)
                | (int(sprite.attr_x_flip) << 5)
                | (int(sprite.attr_palette_number) << 4)
            )
            entries.append(
                {
                    "x": int(sprite.x),
                    "y": int(sprite.y),
                    "tile": (int(sprite.tile_identifier) - ANIM_BASE_TILE_ID) & 0xFF,
                    "attributes": attributes,
                }
            )
        return entries

    def capture(self, move_id: int, side: str, evidence_dir: Path | None) -> dict[str, Any]:
        pyboy = self.pyboy
        pyboy.load_state(io.BytesIO(self.stable_state))
        ctx = self.context
        ctx.requested_id = move_id
        ctx.whose_turn = 0 if side == "player" else 1
        ctx.started = False
        ctx.finished = False
        ctx.resolved_id = None

        memory = pyboy.memory
        player_speed = self._addr("wBattleMonSpeed")
        enemy_speed = self._addr("wEnemyMonSpeed")
        fast, slow = (player_speed, enemy_speed) if side == "player" else (enemy_speed, player_speed)
        memory[fast] = 0xFF
        memory[fast + 1] = 0xFF
        memory[slow] = 0x00
        memory[slow + 1] = 0x01
        memory[self._addr("wOptions")] &= 0x7F

        # Enter FIGHT's move list. The second A below is the semantic trigger.
        self.tap(pyboy, "a", 30)
        previous = np.asarray(pyboy.screen.ndarray).copy()

        baseline: np.ndarray | None = None
        pixel_trace: list[dict[str, Any]] = []
        oam_trace: list[list[dict[str, int]]] = []
        pyboy.button_press("a")
        released = False
        for tick_index in range(1, self.max_frames + 1):
            pyboy.tick()
            if tick_index == 2:
                pyboy.button_release("a")
                released = True
            current = np.asarray(pyboy.screen.ndarray).copy()
            if ctx.started:
                if baseline is None:
                    baseline = previous
                    if evidence_dir is not None:
                        evidence_dir.mkdir(parents=True, exist_ok=True)
                        Image.fromarray(baseline).save(evidence_dir / "frame-000000.png")
                observation = pixel_observation(current, baseline)
                pixel_trace.append(observation)
                oam_trace.append(self._visible_oam())
                if evidence_dir is not None:
                    Image.fromarray(current).save(
                        evidence_dir / f"frame-{len(pixel_trace):06}.png"
                    )
                if ctx.finished:
                    break
            previous = current
        else:
            raise RuntimeError(
                f"reference move {move_id} ({side}) did not finish within {self.max_frames} frames"
            )
        if not released:
            pyboy.button_release("a")
        if baseline is None or not pixel_trace:
            raise RuntimeError(f"reference move {move_id} ({side}) never reached MoveAnimation")
        result = summarize_capture(
            pixel_trace,
            oam_trace,
            resolved_animation_id=ctx.resolved_id,
        )
        if evidence_dir is not None:
            write_json(evidence_dir / "manifest.json", result)
        return result

    def close(self) -> None:
        self.pyboy.stop(save=False)
        self._temp.cleanup()


def load_current_capture(path: Path) -> dict[str, Any]:
    manifest = json.loads((path / "manifest.json").read_text())
    pixel_trace: list[dict[str, Any]] = []
    rendered_oam: list[list[dict[str, int]]] = []
    source_oam: list[list[dict[str, int]]] = []
    baseline = None
    if manifest["frames"][0]["png"] is not None:
        baseline = np.asarray(Image.open(path / manifest["frames"][0]["png"]).convert("RGBA"))
    for frame in manifest["frames"][1:]:
        if "pixel" in frame:
            pixel_trace.append(
                {
                    "delta_crc32": frame["pixel"]["delta_crc32"],
                    "delta_adler32": frame["pixel"]["delta_adler32"],
                    "changed_pixels": frame["pixel"]["changed_pixels"],
                    "bbox": frame["pixel"]["bbox"],
                }
            )
        else:
            if baseline is None:
                raise RuntimeError("legacy current capture has no baseline PNG")
            image = np.asarray(Image.open(path / frame["png"]).convert("RGBA"))
            pixel_trace.append(pixel_observation(image, baseline))
        rendered_oam.append(frame["state"]["oam"])
        source_oam.append(frame["state"].get("source_oam", frame["state"]["oam"]))
    result = summarize_capture(
        pixel_trace,
        rendered_oam,
        source_oam=source_oam,
    )
    result["renderer_move_name"] = manifest["move"]
    return result


def capture_current(
    binary: Path,
    move_id: int,
    side: str,
    max_frames: int,
    evidence_dir: Path | None,
) -> dict[str, Any]:
    if evidence_dir is None:
        temporary = tempfile.TemporaryDirectory(prefix="move-anim-current-")
        target = Path(temporary.name) / "capture"
    else:
        temporary = None
        target = evidence_dir
    command = [
        str(binary),
        "move-animation-frames",
        "--move-id",
        str(move_id),
        "--side",
        side,
        "--output-dir",
        str(target),
        "--max-frames",
        str(max_frames),
    ]
    if evidence_dir is None:
        command.append("--manifest-only")
    completed = subprocess.run(command, text=True, capture_output=True)
    if completed.returncode != 0:
        raise RuntimeError(
            f"current capture failed for move {move_id} ({side}):\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    result = load_current_capture(target)
    if temporary is not None:
        temporary.cleanup()
    return result


def compare_side(reference: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    reference_pixels = expanded_rle(reference["pixel_delta_rle"])
    current_pixels = expanded_rle(current["pixel_delta_rle"])
    reference_oam = expanded_rle(reference["rendered_oam_rle"])
    current_oam = expanded_rle(current["rendered_oam_rle"])
    duration_match = reference["duration_frames"] == current["duration_frames"]
    rendered_oam_match = reference_oam == current_oam
    dynamic_mask_match = reference_pixels == current_pixels
    source_geometry_match = compressed_values(reference["rendered_oam_rle"]) == compressed_values(
        current["source_oam_rle"]
    )
    reference_visible = visible_keyframes(reference["rendered_oam_rle"])
    current_source_visible = visible_keyframes(current["source_oam_rle"])
    visible_source_match = reference_visible == current_source_visible
    visible_source_match_after_offset = reference_visible == subtract_hardware_offsets(
        current_source_visible
    )
    issues: list[str] = []
    diagnostics: list[str] = []
    if not duration_match:
        issues.append("duration")
    if not rendered_oam_match:
        issues.append("rendered_oam")
    if not dynamic_mask_match:
        issues.append("dynamic_pixels")
    if source_geometry_match and not rendered_oam_match:
        diagnostics.append("frontend_oam_drop_or_cadence")
    if (
        reference["max_rendered_objects"] > 0
        and current["max_rendered_objects"] == 0
        and current["max_source_objects"] > 0
    ):
        diagnostics.append("source_oam_generated_but_not_rendered")
    if visible_source_match_after_offset and not visible_source_match:
        diagnostics.append("source_oam_shifted_plus_8_x_plus_16_y")
    return {
        "verdict": "PASS" if not issues else "FAIL",
        "reference_frames": reference["duration_frames"],
        "current_frames": current["duration_frames"],
        "frame_delta": current["duration_frames"] - reference["duration_frames"],
        "duration_match": duration_match,
        "rendered_oam_match": rendered_oam_match,
        "dynamic_mask_match": dynamic_mask_match,
        "source_geometry_match_ignoring_holds": source_geometry_match,
        "visible_source_keyframes_match": visible_source_match,
        "visible_source_keyframes_match_after_hardware_offset": visible_source_match_after_offset,
        "issues": issues,
        "diagnostics": diagnostics,
    }


def compact_summary(summary: dict[str, Any]) -> dict[str, Any]:
    compact = json.loads(json.dumps(summary))
    for move in compact["results"]:
        for side in move["sides"].values():
            side["reference"] = {
                key: side["reference"].get(key)
                for key in (
                    "duration_frames",
                    "trace_sha1",
                    "max_rendered_objects",
                    "resolved_animation_id",
                )
            }
            side["current"] = {
                key: side["current"].get(key)
                for key in (
                    "duration_frames",
                    "trace_sha1",
                    "max_rendered_objects",
                    "max_source_objects",
                    "renderer_move_name",
                )
            }
    compact["frame_trace_archive"] = "frame-traces.json.gz"
    return compact


def source_commit(source: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(source), "rev-parse", "HEAD"],
        check=True,
        text=True,
        capture_output=True,
    ).stdout.strip()


def markdown_report(summary: dict[str, Any]) -> str:
    totals = summary["totals"]
    side_results = [
        side
        for move in summary["results"]
        for side in move["sides"].values()
    ]
    shorter = sum(side["frame_delta"] < 0 for side in side_results)
    equal = sum(side["frame_delta"] == 0 for side in side_results)
    longer = sum(side["frame_delta"] > 0 for side in side_results)
    lines = [
        "# 全技能动画逐帧差分审计",
        "",
        f"- 分支基线：`{summary['provenance']['current_git_base']}`",
        f"- 原作源码：`pret/pokered@{summary['provenance']['reference_source_commit']}`",
        f"- 原作正式 Red ROM SHA-1：`{summary['provenance']['reference_rom_sha1']}`",
        f"- 确定性布置用 DEBUG ROM SHA-1：`{summary['provenance']['setup_rom_sha1']}`（只负责进入战斗，不产生被比较帧）",
        f"- 范围：{totals['moves']} 个技能 × {len(summary['scope']['sides'])} 个攻方视角 = {totals['sides']} 条轨迹；每条轨迹重复 {summary['scope']['repeat']} 次",
        "- 语义边界：`MoveAnimation` 入口到 `.animationFinished`，强制 `wAnimationType = 0`，不含通用命中反馈",
        "",
        "## 结论",
        "",
        f"- PASS：{totals['pass']} 个技能",
        f"- FAIL：{totals['fail']} 个技能",
        f"- 不确定：{totals['inconclusive']} 个技能",
        f"- 双方视角合计：{totals['side_pass']} PASS / {totals['side_fail']} FAIL / {totals['side_inconclusive']} 不确定",
        "",
        "判定门槛为：原作与当前实现的持续帧数、逐帧实际 OAM、逐帧相对基线的像素变化掩码全部相同，且两次采样可重复。任一通道不同即为 FAIL。像素通道比较各自相对动画前一帧的变化掩码，排除了纯静态区域；调色板和位移效果仍会取样各自底图，因此仅有像素差异时还要复核连续帧。",
        "",
        "## 汇总诊断",
        "",
        f"- 时长不同：{totals['duration_mismatch']}/{totals['sides']}；当前更短 {shorter} 条、更长 {longer} 条、相同 {equal} 条。",
        f"- 实际 OAM 不同：{totals['rendered_oam_mismatch']}/{totals['sides']}。其中 {totals['source_oam_generated_but_not_rendered']} 条在播放器内部已生成对象，但前端一帧也没有画出对象。",
        f"- 动态像素掩码不同：{totals['dynamic_mask_mismatch']}/{totals['sides']}。唯一时长和 OAM 都相同的是 `TAKE_DOWN` 两侧，但连续帧中的位移/闪屏相位仍不同，所以不是 PASS。",
        f"- 坐标证据：{totals['visible_source_keyframes_match_after_hardware_offset']}/{totals['sides']} 条轨迹的可见源 OAM 只有在统一减去 X=8、Y=16 后才与原作关键帧相合。",
        "",
        "## 已定位的共性原因",
        "",
        "1. `AnimationPlayer` 明确要求调用方在 `WaitDelay` 后等待指定帧数；当前 `advance_move_animation` 却丢弃 `frames` 并把 `anim_wait` 设为 0。",
        "2. 前端只在 `Playing` 分支复制 OAM；带延时的正常帧返回 `WaitDelay`，因此这些帧虽然存在于播放器缓冲区，却没有进入实际渲染层。",
        "3. 原作基准坐标直接写入硬件 OAM；当前播放器把同一数值标作屏幕坐标，未扣除硬件的 X+8/Y+16 偏移。",
        "4. 原作每个子动画都会通过 `CopyVideoData` 上传 64/79 个图块（每帧 8 个），并在多数 frame block 后执行额外的 OAM 清理帧；当前状态机没有建模这些 VBlank 开销。",
        "",
        "上述四点是跨技能的共性缺陷，不代表修正它们后即可直接宣告全部通过；特殊效果的逐帧相位仍需用同一套审计重新验证。",
        "",
        "源码定位：当前前端的 [`advance_move_animation`](../../../../crates/pokered-app/src/render/battle.rs#L1186-L1223)；锁定依赖中的 [`AnimationPlayer` 调用约定](https://github.com/liuyanghejerry/dotzuki/blob/88f1fccd72c62fcb51ce036e5d2b211e37ac9051/workspace/crates/dotzuki-renderer/src/battle_anim/player.rs#L31-L42) 与 [`WaitDelay` 返回](https://github.com/liuyanghejerry/dotzuki/blob/88f1fccd72c62fcb51ce036e5d2b211e37ac9051/workspace/crates/dotzuki-renderer/src/battle_anim/player.rs#L344-L354)；原作 [`PlayAnimation` / `PlaySubanimation`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/engine/battle/animations.asm#L164-L268) 与 [`CopyVideoData`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/copy2.asm#L62-L111)。",
        "",
        "## 逐技能结果",
        "",
        "| ID | 技能 | 玩家视角（原作→当前） | 敌方视角（原作→当前） | 总结 |",
        "|---:|---|---|---|---|",
    ]
    for move in summary["results"]:
        cells = []
        for side in ("player", "enemy"):
            result = move["sides"].get(side)
            if result is None:
                cells.append("—")
            else:
                cells.append(
                    f"{result['verdict']} ({result['reference_frames']}→{result['current_frames']})"
                )
        lines.append(
            f"| {move['move_id']} | `{move['move']}` | {cells[0]} | {cells[1]} | {move['verdict']} |"
        )
    lines.extend(
        [
            "",
            "## 可复核数据",
            "",
            "`summary.json` 是紧凑索引；完整的逐帧变化哈希、变化像素包围盒、实际 OAM、播放器源 OAM 及其 RLE 计数位于 `frame-traces.json.gz`。`evidence/` 保存抽样技能的原作与当前实现连续 PNG 帧；它们不是挑选关键帧，而是从语义入口到结束的完整窗口。",
            "",
            "复现入口为 `scripts/move_animation_differential.py`；脚本会拒绝非锁定 SHA-1 的 ROM、符号文件或非锁定 commit 的 pret 源码。",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-rom", type=Path, required=True)
    parser.add_argument("--setup-rom", type=Path, required=True)
    parser.add_argument("--reference-symbols", type=Path, required=True)
    parser.add_argument("--reference-source", type=Path, required=True)
    parser.add_argument("--current-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--moves", default="all", help="all, comma list, or inclusive ranges")
    parser.add_argument("--sides", choices=("both", "player", "enemy"), default="both")
    parser.add_argument("--repeat", type=int, default=2)
    parser.add_argument("--max-frames", type=int, default=3000)
    parser.add_argument(
        "--keep-frames",
        default="1,57,89,120,153",
        help="move ids whose run-1 continuous PNG windows are retained",
    )
    args = parser.parse_args()

    require_pinned(args.reference_rom, PINNED_ROM_SHA1, "reference ROM")
    require_pinned(args.setup_rom, PINNED_SETUP_ROM_SHA1, "setup ROM")
    require_pinned(args.reference_symbols, PINNED_SYMBOLS_SHA1, "reference symbols")
    commit = source_commit(args.reference_source)
    if commit != PINNED_SOURCE_COMMIT:
        raise RuntimeError(
            f"reference source commit mismatch: expected {PINNED_SOURCE_COMMIT}, got {commit}"
        )
    if not args.current_binary.is_file():
        raise RuntimeError(f"current binary does not exist: {args.current_binary}")
    if args.repeat < 2:
        raise ValueError("repeat must be at least 2 before a PASS can be reported")
    require_empty_output(args.output)

    names = parse_move_names(args.reference_source)
    move_ids = parse_id_set(args.moves, MOVE_COUNT)
    keep_ids = set(parse_id_set(args.keep_frames, MOVE_COUNT)) if args.keep_frames else set()
    sides = ("player", "enemy") if args.sides == "both" else (args.sides,)
    git_base = subprocess.run(
        ["git", "rev-parse", "master"], check=True, text=True, capture_output=True
    ).stdout.strip()
    summary: dict[str, Any] = {
        "schema": 1,
        "provenance": {
            "reference_rom": str(args.reference_rom.resolve()),
            "reference_rom_sha1": sha1_file(args.reference_rom),
            "setup_rom": str(args.setup_rom.resolve()),
            "setup_rom_sha1": sha1_file(args.setup_rom),
            "reference_symbols": str(args.reference_symbols.resolve()),
            "reference_symbols_sha1": sha1_file(args.reference_symbols),
            "reference_source": str(args.reference_source.resolve()),
            "reference_source_commit": commit,
            "current_binary": str(args.current_binary.resolve()),
            "current_git_base": git_base,
        },
        "scope": {
            "move_ids": move_ids,
            "sides": list(sides),
            "repeat": args.repeat,
            "max_frames": args.max_frames,
            "kept_continuous_frames": sorted(keep_ids & set(move_ids)),
        },
        "comparison": {
            "window": "MoveAnimation entry through MoveAnimation.animationFinished",
            "applying_attack_animation": False,
            "pixel_channel": "per-frame changed-pixel mask relative to each implementation's pre-entry frame",
            "pass_gate": "duration + rendered OAM + dynamic pixel mask exact; all repeats deterministic",
        },
        "results": [],
    }

    reference_runner = ReferenceRunner(
        args.reference_rom, args.setup_rom, args.reference_symbols, args.max_frames
    )
    try:
        for position, move_id in enumerate(move_ids, 1):
            move_result: dict[str, Any] = {
                "move_id": move_id,
                "move": names[move_id - 1],
                "sides": {},
            }
            for side in sides:
                reference_runs = []
                current_runs = []
                for run_index in range(1, args.repeat + 1):
                    keep = move_id in keep_ids and run_index == 1
                    reference_evidence = (
                        args.output / "evidence" / f"move-{move_id:03}" / side / "reference"
                        if keep
                        else None
                    )
                    current_evidence = (
                        args.output / "evidence" / f"move-{move_id:03}" / side / "current"
                        if keep
                        else None
                    )
                    reference_runs.append(
                        reference_runner.capture(move_id, side, reference_evidence)
                    )
                    current_runs.append(
                        capture_current(
                            args.current_binary,
                            move_id,
                            side,
                            args.max_frames,
                            current_evidence,
                        )
                    )
                reference_deterministic = len({r["trace_sha1"] for r in reference_runs}) == 1
                current_deterministic = len({r["trace_sha1"] for r in current_runs}) == 1
                comparison = compare_side(reference_runs[0], current_runs[0])
                if not reference_deterministic or not current_deterministic:
                    comparison["verdict"] = "INCONCLUSIVE"
                    comparison["issues"].append("nondeterministic_repeat")
                comparison.update(
                    {
                        "reference_deterministic": reference_deterministic,
                        "current_deterministic": current_deterministic,
                        "reference_repeat_sha1": [r["trace_sha1"] for r in reference_runs],
                        "current_repeat_sha1": [r["trace_sha1"] for r in current_runs],
                        "reference": reference_runs[0],
                        "current": current_runs[0],
                    }
                )
                move_result["sides"][side] = comparison
            verdicts = {item["verdict"] for item in move_result["sides"].values()}
            move_result["verdict"] = (
                "INCONCLUSIVE"
                if "INCONCLUSIVE" in verdicts
                else "PASS"
                if verdicts == {"PASS"}
                else "FAIL"
            )
            summary["results"].append(move_result)
            write_json(args.output / "summary.partial.json", summary)
            side_text = ", ".join(
                f"{side}={result['verdict']} {result['reference_frames']}→{result['current_frames']}"
                for side, result in move_result["sides"].items()
            )
            print(f"[{position:03}/{len(move_ids):03}] {move_id:03} {names[move_id - 1]}: {side_text}", flush=True)
    finally:
        reference_runner.close()

    side_results = [
        side
        for move in summary["results"]
        for side in move["sides"].values()
    ]
    summary["totals"] = {
        "moves": len(summary["results"]),
        "sides": len(side_results),
        "pass": sum(move["verdict"] == "PASS" for move in summary["results"]),
        "fail": sum(move["verdict"] == "FAIL" for move in summary["results"]),
        "inconclusive": sum(move["verdict"] == "INCONCLUSIVE" for move in summary["results"]),
        "side_pass": sum(side["verdict"] == "PASS" for side in side_results),
        "side_fail": sum(side["verdict"] == "FAIL" for side in side_results),
        "side_inconclusive": sum(side["verdict"] == "INCONCLUSIVE" for side in side_results),
        "duration_mismatch": sum(not side["duration_match"] for side in side_results),
        "rendered_oam_mismatch": sum(not side["rendered_oam_match"] for side in side_results),
        "dynamic_mask_mismatch": sum(not side["dynamic_mask_match"] for side in side_results),
        "source_geometry_match_ignoring_holds": sum(
            side["source_geometry_match_ignoring_holds"] for side in side_results
        ),
        "visible_source_keyframes_match": sum(
            side["visible_source_keyframes_match"] for side in side_results
        ),
        "visible_source_keyframes_match_after_hardware_offset": sum(
            side["visible_source_keyframes_match_after_hardware_offset"]
            for side in side_results
        ),
        "source_oam_generated_but_not_rendered": sum(
            "source_oam_generated_but_not_rendered" in side["diagnostics"]
            for side in side_results
        ),
    }
    trace_bytes = (json.dumps(summary, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    with gzip.GzipFile(
        filename=str(args.output / "frame-traces.json.gz"), mode="wb", mtime=0
    ) as archive:
        archive.write(trace_bytes)
    compact = compact_summary(summary)
    write_json(args.output / "summary.json", compact)
    (args.output / "report.md").write_text(markdown_report(compact))
    partial = args.output / "summary.partial.json"
    partial.unlink(missing_ok=True)
    print(json.dumps(summary["totals"], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
