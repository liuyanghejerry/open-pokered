#!/usr/bin/env python3
"""Build Pokemon's mobile library and export the engine-owned HarmonyOS host."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine-checkout', type=Path, required=True,
                        help='dotzuki checkout containing the shared-mobile-host changes')
    parser.add_argument('--native-sdk', type=Path, required=True,
                        help='HarmonyOS SDK native directory containing llvm/ and sysroot/')
    parser.add_argument('--out', type=Path, default=Path('dist/harmony'))
    parser.add_argument('--version', choices=['red', 'blue'], default='red')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    engine = args.engine_checkout.resolve() / 'workspace'
    sdk = args.native_sdk.resolve()
    if not (root / 'gfx/blocksets').is_dir():
        parser.error('missing gfx assets; run scripts/fetch-gfx.sh first')
    if not (engine / 'crates/dotzuki-mobile/Cargo.toml').is_file():
        parser.error('engine checkout does not contain dotzuki-mobile')
    linker = sdk / 'llvm/bin/aarch64-unknown-linux-ohos-clang'
    if not linker.is_file():
        parser.error('native SDK does not provide the ARM64 OHOS Clang wrapper')
    output = args.out.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error('output must be empty; use a new directory to preserve host edits')
    generated = root / 'target/harmony'
    generated.mkdir(parents=True, exist_ok=True)
    patch = generated / 'engine.toml'
    lines = ['[patch."https://github.com/liuyanghejerry/dotzuki"]']
    # Only patch engine crates consumed by this workspace; no hardcoded user paths.
    consumed = set()
    for manifest in (root / 'crates').glob('*/Cargo.toml'):
        for line in manifest.read_text().splitlines():
            if line.startswith('dotzuki-') and ' = ' in line:
                consumed.add(line.split(' = ', 1)[0])
    for manifest in sorted((engine / 'crates').glob('*/Cargo.toml')):
        name = tomllib.loads(manifest.read_text())['package']['name']
        if name in consumed:
            lines.append(f'{name} = {{ path = {json.dumps(str(manifest.parent))} }}')
    patch.write_text('\n'.join(lines) + '\n')
    env = os.environ.copy()
    env['CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER'] = str(linker)
    env['CC_aarch64_unknown_linux_ohos'] = str(linker)
    env['AR_aarch64_unknown_linux_ohos'] = str(sdk / 'llvm/bin/llvm-ar')
    subprocess.run(['cargo', 'build', '--release', '--target', 'aarch64-unknown-linux-ohos',
                    '--config', str(patch), '-p', 'pokered-mobile'], cwd=root, env=env, check=True)
    target = Path(env.get('CARGO_TARGET_DIR', str(root / 'target')))
    if not target.is_absolute():
        target = root / target
    library = target / 'aarch64-unknown-linux-ohos/release/libpokered_mobile.a'
    subprocess.run([sys.executable, str(engine / 'scripts/export-mobile-host.py'),
                    '--platform', 'harmony',
                    '--library', str(library), '--init', f'pokered:{args.version}:v1',
                    '--out', str(output), '--title', 'Pokered', '--bundle', 'com.pokered.mobile'], check=True)
    print(f'Open {output} in DevEco Studio, or build entry with hvigor assembleHap.')
    print(f'Local integration Cargo configuration: {patch}')


if __name__ == '__main__':
    main()
