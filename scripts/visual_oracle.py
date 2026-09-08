#!/usr/bin/env python3
"""Small, real-ROM visual differential experiment within m02. Never downloads ROMs.

Requires: pip install -r scripts/visual_oracle_requirements.txt
python scripts/visual_oracle.py --rom /path/pokered.gbc --binary target/debug/pokered-app --output /tmp/visual-oracle
"""
import argparse
import hashlib
import importlib.metadata
import io
import json
from pathlib import Path
import subprocess
import socket
from debug_drive import DebugClient
import tempfile
import time

import numpy as np
from PIL import Image, ImageDraw
from pyboy import PyBoy

ROOT = Path(__file__).resolve().parents[1]

RED_SHA1 = 'ea9bcae617fdf159b045185467ae58b2e4a48b9a'
# Fixed to the verified USA/Europe Red ROM, never guessed for other versions.
ADDR = {'map': 0xd35e, 'x': 0xd362, 'y': 0xd361, 'facing': 0xc109}


def state(p):
    return {key: p.memory[address] for key, address in ADDR.items()}


def shades(im):
    """Map a complete frame's four distinct colors to luminance-ordered indices.

    Ordering is a declared DMG palette assumption; missing/additional colors fail explicitly.
    Color identity is intentionally excluded; geometry stays exact.
    """
    rgb = np.asarray(im.convert('RGB'))
    colors = np.unique(rgb.reshape(-1, 3), axis=0)
    if len(colors) != 4:
        raise ValueError(f'Expected all 4 flat DMG colors, got {len(colors)}')
    colors = sorted(colors.tolist(), key=lambda c: sum(c[i] * (299, 587, 114)[i] for i in range(3)))
    result = np.zeros(rgb.shape[:2], dtype=np.uint8)
    for i, color in enumerate(colors):
        result[np.all(rgb == color, axis=2)] = i
    return result


def difference(a, b):
    if a.shape != b.shape:
        raise ValueError(f'Frame sizes differ: {a.shape} vs {b.shape}')
    mask = a != b
    ys, xs = np.nonzero(mask)
    return {'pixels': int(mask.sum()), 'total': mask.size,
            'fraction': float(mask.mean()),
            'bbox_xyxy': [int(xs.min()), int(ys.min()), int(xs.max()+1), int(ys.max()+1)] if len(xs) else None}


def compare(original, candidate, output, name):
    a, b = shades(original), shades(candidate)
    if len(np.unique(a)) != len(np.unique(b)):
        raise ValueError('Palette cardinalities differ; ordinal normalization is ambiguous')
    result = {'palette_normalized': difference(a, b),
              'raw_rgb': {'fraction': float(np.any(np.asarray(original.convert('RGB')) != np.asarray(candidate.convert('RGB')), axis=2).mean())},
              'regions': {}}
    for region, box in {'upper_map': (0, 0, 160, 48), 'player_band': (0, 48, 160, 96), 'lower_map': (0, 96, 160, 144)}.items():
        x0, y0, x1, y1 = box
        result['regions'][region] = difference(a[y0:y1,x0:x1], b[y0:y1,x0:x1])
    # Diagnostic only: never use registration to waive a camera offset failure.
    offsets = []
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            offsets.append((float((a[8:136,8:152] != b[8+dy:136+dy,8+dx:152+dx]).mean()), dx, dy))
    best = min(offsets)
    result['diagnostic_registration'] = {'candidate_offset_xy': list(best[1:]), 'interior_fraction': best[0], 'used_for_pass': False}
    heat = np.zeros((144,160,3),dtype=np.uint8)
    heat[a != b] = (255,40,60)
    panel = Image.new('RGB',(480,164),'white')
    panel.paste(original.convert('RGB'),(0,20)); panel.paste(candidate.convert('RGB'),(160,20)); panel.paste(Image.fromarray(heat),(320,20))
    draw = ImageDraw.Draw(panel)
    for x,label in [(0,'Original ROM'),(160,'Rust remake'),(320,'Normalized diff')]:draw.text((x+3,3),label,fill='black')
    panel.resize((960,328),Image.Resampling.NEAREST).save(output / f'{name}-comparison.png')
    return result


def run(args):
    started = time.perf_counter()
    output = Path(args.output).resolve()
    if output.exists() and any(output.iterdir()):
        raise ValueError(f'Output must be empty to preserve evidence: {output}')
    output.mkdir(parents=True,exist_ok=True)
    rom = Path(args.rom).resolve(); binary = Path(args.binary).resolve()
    digest = hashlib.sha1(rom.read_bytes()).hexdigest()
    if digest != RED_SHA1:
        raise ValueError(f'Unverified ROM SHA1 {digest}; expected {RED_SHA1}. Build the pinned pret/pokered source.')
    report = {'schema': 1, 'rom_sha1': digest, 'addresses': ADDR,
              'versions': {p:importlib.metadata.version(p) for p in ('pyboy','numpy','pillow')},
              'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'scope': 'm02 RedsHouse2F: real boot + walk, Rust debug-seeded equivalents; not a whole playthrough',
              'scenes': [], 'calibration': {}}
    with tempfile.TemporaryDirectory(prefix='visual-oracle-') as tmp:
        # Isolate ROM from stale .ram/.state and PyBoy's incompatible RGBDS constant-symbol parser.
        local_rom = Path(tmp)/'reference.gb';local_rom.write_bytes(rom.read_bytes())
        p = PyBoy(str(local_rom),window='null',sound_emulated=False,log_level='ERROR')
        try:
            p.set_emulation_speed(0)
            p.tick(500);p.button('start');p.tick(120);p.button('start');p.tick(120)
            # Deliberately bounded bootstrap, not falsely described as semantic control.
            for _ in range(120):p.button('a');p.tick(60)
            p.button('up',2);p.tick(30)
            if state(p) != {'map':38,'x':3,'y':6,'facing':4}:
                raise RuntimeError(f'Bootstrap did not reach bedroom: {state(p)}')
            for name, x in [('bedroom_spawn',3),('bedroom_walk_left',2)]:
                if x == 2:
                    p.button('left',8);p.tick(30);p.button('up',2);p.tick(30)
                observed = state(p)
                if observed != {'map':38,'x':x,'y':6,'facing':4}:
                    raise RuntimeError(f'Semantic alignment failed: {observed}')
                p.tick(30)
                reference = p.screen.image.copy().convert('RGB')
                capture_frame = p.frame_count
                reference.save(output/f'{name}-original.png')
                # Save/restore plus equal advancement tests actual simulator replay noise.
                checkpoint = io.BytesIO();p.save_state(checkpoint)
                p.tick(30);first=p.screen.image.copy()
                checkpoint.seek(0);p.load_state(checkpoint);p.tick(30);second=p.screen.image.copy()
                report['calibration'][name+'_emulator_replay'] = difference(shades(first),shades(second))
                path = output/f'{name}-rust.png'
                cmd = [str(binary),'run','--save',str(Path(tmp)/'empty.sav'),'--skip-intro','--warp',f'RedsHouse2F,{x},6','--no-audio','--screenshot',str(path),'--screenshot-frames','30']
                subprocess.run(cmd,check=True,capture_output=True,text=True,cwd=ROOT)
                candidate = Image.open(path).convert('RGB')
                rerun_path = Path(tmp)/'rust-rerun.png'
                rerun_cmd = cmd.copy();rerun_cmd[rerun_cmd.index('--screenshot')+1] = str(rerun_path)
                subprocess.run(rerun_cmd,check=True,capture_output=True,text=True,cwd=ROOT)
                report['calibration'][name+'_rust_rerun'] = difference(shades(candidate),shades(Image.open(rerun_path)))
                with socket.socket() as available:
                    available.bind(('127.0.0.1',0));port = available.getsockname()[1]
                driver_cmd = [str(binary),'run','--save',str(Path(tmp)/'empty.sav'),'--skip-intro','--warp',f'RedsHouse2F,{x},6','--no-audio','--headless','--debug-port',str(port)]
                process = subprocess.Popen(driver_cmd,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,cwd=ROOT)
                try:
                    client = DebugClient(port)
                    try:
                        client.step(30);rust_state = client.state()
                    finally:client.close()
                    expected = {'map_id':38,'player_x':x,'player_y':6,'player_facing':'Up','screen':'overworld','player_movement_state':'Idle','warp_fade':'Idle'}
                    if any(rust_state.get(k) != v for k,v in expected.items()):
                        raise RuntimeError(f'Rust semantic alignment failed: {rust_state}')
                finally:
                    process.terminate()
                    try:process.wait(timeout=5)
                    except subprocess.TimeoutExpired:process.kill();process.wait()
                entry = {'id': name,'original_state':observed,'rust_seed':{'map':'RedsHouse2F','x':x,'y':6,'facing':'Up'},'rust_capture_command':cmd,'rust_observation_command':driver_cmd,'rust_observed':rust_state,'original_frame':capture_frame}
                entry.update(compare(reference,candidate,output,name));report['scenes'].append(entry)
                # Sensitivity: inject an 8x8 replacement into test image only, never product code.
                base = shades(reference);mutated=base.copy();mutated[16:24,16:24]=(mutated[16:24,16:24]+1)%4
                report['calibration'][name+'_injected_8x8'] = difference(base,mutated)
                # Arbitrary monotonic recolor should disappear after normalization.
                palette=np.array([[0,0,0],[50,70,60],[140,170,150],[230,255,240]],dtype=np.uint8)
                report['calibration'][name+'_palette_only'] = difference(base,shades(Image.fromarray(palette[base])))
        finally:p.stop(save=False)
    report['elapsed_seconds'] = round(time.perf_counter()-started,3)
    report['calibration_pass'] = all(v['pixels'] == (64 if 'injected' in k else 0) for k,v in report['calibration'].items())
    report['verdict'] = 'needs_review' if any(s['palette_normalized']['pixels'] for s in report['scenes']) else 'equal'
    (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
    return 0 if report['calibration_pass'] else 1


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rom',required=True)
    parser.add_argument('--binary',default='target/debug/pokered-app')
    parser.add_argument('--output',default='/tmp/visual-oracle')
    raise SystemExit(run(parser.parse_args()))
