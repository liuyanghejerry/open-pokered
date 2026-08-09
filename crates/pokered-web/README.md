# pokered-web — SPA Deployment Guide

This crate provides a standalone SPA (Single Page Application) build of Pokémon Red/Blue that can be deployed to any static web server.

## Prerequisites

1. **wasm-pack** — WASM build tool
   ```bash
   cargo install wasm-pack
   ```

2. **wasm32-unknown-unknown target**
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

## Building

From this directory (`crates/pokered-web`):

```bash
# Build for production (optimized, smaller WASM)
./build-web.sh release

# Build for development (faster compile, larger WASM, better debugging)
./build-web.sh debug
```

The build script will:
1. Compile the Rust code to WASM
2. Generate JavaScript glue code with `wasm-bindgen`
3. Copy static assets (HTML, CSS) to the output directory
4. Create a ready-to-deploy `pkg/` directory

## Output Structure

After building, the `pkg/` directory contains:

```
pkg/
├── pokered_web.js         # JavaScript glue code
├── pokered_web_bg.wasm    # WASM binary
├── pokered_web_bg.js      # WASM loader (internal)
├── index.html             # Main HTML page
└── (other web assets)
```

## Local Testing

Serve the `pkg/` directory with any static file server:

```bash
cd pkg

# Python
python3 -m http.server 8080

# Node.js (if you have npx)
npx serve

# Rust (if you have miniserve installed)
miniserve .
```

Open `http://localhost:8080` in your browser.

**Browser Requirements:**
- Chrome 113+ / Edge 113+ (WebGPU support)
- Firefox Nightly with WebGPU flag enabled
- Safari 15+ (WebGL2 fallback)

## Deployment

### GitHub Pages

1. Build the project:
   ```bash
   ./build-web.sh release
   ```

2. Create a `gh-pages` branch:
   ```bash
   git checkout -b gh-pages
   ```

3. Copy the `pkg/` contents to the branch root:
   ```bash
   rm -rf *
   cp -r pkg/* .
   ```

4. Commit and push:
   ```bash
   git add .
   git commit -m "Deploy pokered-web SPA"
   git push -f origin gh-pages
   ```

5. Enable GitHub Pages in your repo settings (Settings → Pages → Source: `gh-pages` branch)

### Netlify / Vercel / Cloudflare Pages

1. Build the project locally:
   ```bash
   ./build-web.sh release
   ```

2. Deploy the `pkg/` directory:
   - **Netlify**: Drag & drop `pkg/` folder to [netlify.com/drop](https://app.netlify.com/drop)
   - **Vercel**: `vercel deploy --prod pkg`
   - **Cloudflare Pages**: Upload `pkg/` via dashboard or use Wrangler

### Any Static Web Server

Copy the `pkg/` directory to your web server's document root:

```bash
scp -r pkg/* yourserver:/var/www/html/pokered/
```

Access at `https://yourserver.com/pokered/`

## Build Configuration

The build uses these defaults:
- **Target**: `wasm32-unknown-unknown`
- **Output**: `pkg/` directory
- **Mode**: `web` (generates ES module compatible JS)
- **No TypeScript**: Plain JS output (no `.d.ts` files)

To customize, edit `build-web.sh` or run `wasm-pack build` directly:

```bash
# Custom output directory
wasm-pack build --release --target web --out-dir custom-output

# Enable TypeScript generation
wasm-pack build --release --target web --out-dir pkg

# Debug build with TypeScript
wasm-pack build --dev --target web --out-dir pkg
```

## File Size Optimization

Release builds are optimized for size:
- WASM binary is stripped and minified
- JavaScript glue code is compact
- No debug symbols

Typical size:
- `pokered_web_bg.wasm`: ~2-5 MB (depends on included features)
- `pokered_web.js`: ~10-20 KB

To further reduce size:
1. Disable unused features in `Cargo.toml`
2. Use `wasm-opt` (if installed): `wasm-opt -Oz pkg/pokered_web_bg.wasm -o pkg/pokered_web_bg.wasm`

## Development Workflow

For rapid iteration:

```bash
# Build in debug mode (faster)
./build-web.sh debug

# Serve locally
cd pkg && python3 -m http.server 8080

# Edit code, rebuild, refresh browser
```

Debug builds include:
- Better error messages
- Stack traces in WASM
- No optimization (larger size)

## Troubleshooting

### "wasm-pack not found"
Install: `cargo install wasm-pack`

### "wasm32-unknown-unknown target not found"
Add target: `rustup target add wasm32-unknown-unknown`

### "Failed to load WASM module"
- Check browser console for errors
- Ensure all files are served with correct MIME types
- Web servers must serve `.wasm` as `application/wasm`

### "Canvas not found"
The HTML must have `<canvas id="game-canvas">`. Check `index.html`.

### "GPU initialization failed"
- Browser may not support WebGPU/WebGL2
- Try Chrome/Edge 113+
- Check GPU driver updates

### Slow performance
- Use `release` build
- Ensure GPU acceleration is enabled in browser

## Architecture

This crate uses:
- **wasm-bindgen** — Rust ↔ JavaScript bridge
- **pixels** — GPU rendering (WebGPU/WebGL2 via wgpu)
- **winit** — Window/event handling with web canvas support
- **console_error_panic_hook** — Rust panic → JavaScript console
- **console_log** — Rust logs → JavaScript console

The game runs at ~60 FPS (Game Boy VBlank rate) with GPU-accelerated rendering.

## Save Files

On the web build the in-game **SAVE** option persists progress to the
browser's `window.localStorage` under the key `pokered.save`. The save
is stored as a JSON serialization of `SaveData` so it preserves both the
SRAM region and the script-engine flags. The game automatically reloads
the save (and enables the **CONTINUE** menu entry) on the next visit to
the same origin.

Notes:
- Storage is per-origin and per-browser-profile. Clearing site data
  will erase the save.
- Private/Incognito modes or browsers with `localStorage` disabled fall
  back to running without persistence; saving emits a warning to the
  console in that case.
- To wipe an existing save, run `localStorage.removeItem('pokered.save')`
  in the browser DevTools console and reload the page.

## License

MIT. See workspace root LICENSE file.