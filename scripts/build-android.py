#!/usr/bin/env python3
"""Build Pokemon's mobile library and export the engine-owned Android host."""
import argparse
import os
from pathlib import Path
import platform
import subprocess
import sys


def find_ndk(explicit: Path | None) -> Path | None:
    if explicit:
        return explicit.resolve()
    if value := os.environ.get('ANDROID_NDK_HOME'):
        return Path(value).resolve()
    sdk = os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT')
    if not sdk:
        return None
    versions = sorted((Path(sdk) / 'ndk').glob('*'))
    return versions[-1].resolve() if versions else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine-checkout', type=Path, required=True,
                        help='dotzuki checkout containing the Android host template')
    parser.add_argument('--ndk', type=Path,
                        help='Android NDK root; defaults to ANDROID_NDK_HOME or the SDK')
    parser.add_argument('--out', type=Path, default=Path('dist/android'))
    parser.add_argument('--version', choices=['red', 'blue'], default='red')
    parser.add_argument('--assemble', action='store_true',
                        help='assemble a debug APK with android/gradlew after export')
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    engine = args.engine_checkout.resolve() / 'workspace'
    ndk = find_ndk(args.ndk)
    if not (root / 'gfx/blocksets').is_dir():
        parser.error('missing gfx assets; run scripts/fetch-gfx.sh first')
    if not (engine / 'crates/dotzuki-mobile/Cargo.toml').is_file():
        parser.error('engine checkout does not contain dotzuki-mobile')
    if ndk is None or not ndk.is_dir():
        parser.error('Android NDK not found; pass --ndk or set ANDROID_NDK_HOME')

    host = 'darwin-x86_64' if platform.system() == 'Darwin' else 'linux-x86_64'
    ndk_bin = ndk / f'toolchains/llvm/prebuilt/{host}/bin'
    linker = ndk_bin / 'aarch64-linux-android26-clang'
    if not linker.is_file():
        parser.error(f'Android NDK compiler missing: {linker}')

    output = args.out.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error('output must be empty; use a new directory to preserve host edits')

    env = os.environ.copy()
    env['CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER'] = str(linker)
    env['AR_aarch64_linux_android'] = str(ndk_bin / 'llvm-ar')
    subprocess.run(
        [
            'cargo', 'build', '--release', '--target', 'aarch64-linux-android',
            '-p', 'pokered-mobile',
        ],
        cwd=root,
        env=env,
        check=True,
    )
    target = Path(env.get('CARGO_TARGET_DIR', str(root / 'target')))
    if not target.is_absolute():
        target = root / target
    library = target / 'aarch64-linux-android/release/libpokered_mobile.a'
    subprocess.run(
        [
            sys.executable, str(engine / 'scripts/export-mobile-host.py'),
            '--platform', 'android',
            '--library', str(library),
            '--init', f'pokered:{args.version}:v1',
            '--out', str(output),
            '--title', 'Pokered',
            '--bundle', 'com.pokered.mobile',
        ],
        check=True,
    )
    print(f'Open {output} in Android Studio.')

    if args.assemble:
        gradlew = root / 'android/gradlew'
        subprocess.run(
            [str(gradlew), '--project-dir', str(output), 'assembleDebug', '--no-daemon'],
            cwd=root,
            env=env,
            check=True,
        )
        apk = output / 'app/build/outputs/apk/debug/app-debug.apk'
        print(f'APK: {apk}')


if __name__ == '__main__':
    main()
