# Desktop Packaging Research — macOS & Windows

> Branch: `research/desktop-packaging`
> Date: 2026-05-28

## Current State

- **Desktop binary**: `pokered-app` — pure Rust, single executable (`winit` 0.30 + `pixels` 0.15 → wgpu)
- **GPU backends**: Metal (macOS), DX12/DX11 (Windows), Vulkan (Linux) — zero extra DLLs, all system frameworks
- **Audio**: `cpal` — CoreAudio (macOS) / WASAPI (Windows) — no extra deps
- **No existing packaging infra**: no `.icns`/`.ico` icons, no `Info.plist` for macOS desktop, no `.rc`/`.manifest` for Windows, no `.dmg`/`.msi` scripts
- **Assets**: `gfx/` directory loaded from filesystem at runtime (not embedded for desktop builds)
- **CI**: all native builds on `ubuntu-latest` only, no `macos-latest` or `windows-latest` runner for desktop

---

## macOS Packaging

### Recommended Stack

| Step | Tool | Why |
|------|------|-----|
| `.app` bundle | **`cargo-bundle` 0.10.0** | Creates proper `Contents/{MacOS,Resources,Info.plist}` from `Cargo.toml` metadata |
| `.dmg` | **`hdiutil`** (built-in) | Zero deps, CI-friendly; `create-dmg` if branding needed |
| Code signing | **`codesign` + `xcrun notarytool`** | Required for Gatekeeper on 10.15+ |
| App icon | **`iconutil` + `sips`** | Convert PNG → `.icns`, no Xcode needed |

### `.app` Bundle Layout

```
PokemonRed.app/
└── Contents/
    ├── Info.plist
    ├── MacOS/
    │   └── pokered-app          # compiled binary
    ├── Resources/
    │   ├── icon.icns
    │   └── gfx/                 # game assets (sprites, tilesets)
    └── _CodeSignature/          (after codesign)
```

### cargo-bundle Config

```toml
# Cargo.toml of pokered-app
[package.metadata.bundle]
name = "PokemonRed"
identifier = "com.pokered.desktop"
icon = ["resources/icon.icns"]
version = "0.1.0"
copyright = "Copyright (c) 2026"
category = "public.app-category.games"
short_description = "Pokémon Red reimplementation"
osx_minimum_system_version = "11.0"
resources = ["../../gfx/**/*"]
```

### Dmg Creation

```bash
# Simple CI approach (built-in tools only)
hdiutil create -volname "PokemonRed" \
  -srcfolder target/release/bundle/osx/PokemonRed.app \
  -ov -format UDZO "PokemonRed-aarch64.dmg"
```

### Code Signing (for Distribution)

Requires Apple Developer account ($99/yr). In CI:
1. Import `.p12` cert into temp keychain
2. `codesign --deep --force --timestamp --options runtime --entitlements entitlements.plist --sign "Developer ID Application: ..."`
3. Notarize via `xcrun notarytool submit`

Without signing → app works locally but shows Gatekeeper warning on other machines.

### Key Detail: Metal Entitlements

wgpu/Metal works with hardened runtime using minimal entitlements — just `cs.disable-library-validation` is needed.

---

## Windows Packaging

### Recommended Stack

| Step | Tool | Why |
|------|------|-----|
| Zip archive | **`7z`** or `zip` | Single static-linked exe + assets, no installer needed |
| App icon | **`.ico`** embedded via `winres` | Adds taskbar icon to the `.exe` itself |
| Code signing (optional) | **`signtool`** + Azure Trusted Signing | Only needed to suppress SmartScreen |

### Why Zip (Not MSI/NSIS)

- **`pokered-app`** is a pure Rust static-linked binary — no .NET, no VC++ Runtime, no DLL dependencies
- wgpu uses system DX12/DX11 APIs (built into Win10+), cpal uses WASAPI (built-in)
- The distribution is just: `pokered-app.exe` + `gfx/` folder
- Users unzip and run — zero install friction
- MSI/NSIS is overkill for a single binary + static assets

### Cross-Compilation Note

- `x86_64-pc-windows-msvc` requires a **`windows-latest`** runner — cannot cross-compile from Linux/macOS
- For CI: just run `cargo build --release -p pokered-app` on `windows-latest`, then zip the result

### Output Layout

```
pokered-app-win64.zip/
├── pokered-app.exe
└── gfx/
    ├── pokemon/
    ├── tilesets/
    ├── ...
```

---

## Cross-Compilation Strategy

### For CI (Release Workflow)

| Platform | Runner | Target |
|----------|--------|--------|
| macOS ARM | `macos-14` (M2) | `aarch64-apple-darwin` |
| macOS Intel | `macos-14` (cross from ARM via Rosetta) | `x86_64-apple-darwin` |
| Windows | `windows-latest` | `x86_64-pc-windows-msvc` |

> `macos-14` can build **both** macOS targets natively — Rosetta allows x86_64 toolchain to run on ARM hardware.

### Asset Bundling

Current desktop build loads `gfx/` from filesystem at runtime. For bundled distribution, options:
1. **Set asset root relative to executable**: `ResourceManager` resolves `gfx/` relative to `std::env::current_exe()` — works inside `.app/Contents/Resources/gfx/`
2. **Embed assets**: Extend `build.rs` to embed for desktop too (currently only wasm/android/ios)
3. **Both**: binary first checks embedded, falls back to filesystem

**Recommended**: Option 1 (relative path) is simplest and matches `.app` bundle convention.

---

## CI Job Design (Draft)

Trigger: on push to master (alongside existing jobs)

```yaml
release-desktop:
  needs: [detect-changes]
  if: ${{ needs.detect-changes.outputs.pokered-ios == 'true' || ... }}
  strategy:
    matrix:
      include:
        - os: macos-14
          target: aarch64-apple-darwin
        - os: macos-14
          target: x86_64-apple-darwin
        - os: windows-latest
          target: x86_64-pc-windows-msvc
  runs-on: ${{ matrix.os }}
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with: { targets: ${{ matrix.target }} }
    - uses: Swatinem/rust-cache@v2
    - run: cargo build --release -p pokered-app --target ${{ matrix.target }}

    - if: runner.os == 'macOS'
      run: |
        cargo install cargo-bundle
        cargo bundle --release --format osx --target ${{ matrix.target }}
        hdiutil create -volname "PokemonRed" \
          -srcfolder target/${{ matrix.target }}/release/bundle/osx/ \
          -ov -format UDZO "PokemonRed-${{ matrix.target }}.dmg"

    # Windows: zip (exe + assets)
    - if: runner.os == 'Windows'
      run: |
        mkdir -p staging/gfx
        cp target/${{ matrix.target }}/release/pokered-app.exe staging/
        cp -r gfx staging/
        Compress-Archive -Path staging/* -DestinationPath PokemonRed-${{ matrix.target }}.zip
        Remove-Item -Recurse staging

    # macOS: bundle → dmg
```

---

## Open Questions / Next Steps

1. **App icon**: Need to create an app icon (PNG → .icns + .ico)
2. **Info.plist**: Create macOS `Info.plist` for the desktop `.app`
3. **Resource path**: Confirm `ResourceManager` can resolve assets relative to the bundled `.app`
4. **Release vs CI**: Keep separate — CI builds verify compilation, release builds produce signed/packaged artifacts only on tag push
5. **Signing certs**: Need Apple Developer Program account for macOS notarization; optional for Windows
