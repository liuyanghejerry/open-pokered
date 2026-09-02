import { defineConfig, type Plugin, type ViteDevServer } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
// AI assistant / action framework / sprite pipeline, ported from dotzuki-editor
// (tools/dotzuki-editor/server) as Vite dev-server middleware.
import { registerBuiltinActions } from './server/actions'
import { registerProject } from './server/api/routes/project'
import { registerAi } from './server/api/routes/ai'
import { registerSprites } from './server/api/routes/sprites'
// Pokered data routes (/api/maps*, /api/trainers*, /gfx, /wasm, …). These used
// to be inline middleware in this file; they now live in server/pokeredRoutes.ts
// so the Electron production api-server (electron/api-server.ts) mounts the
// exact same handlers.
import { registerPokeredRoutes } from './server/pokeredRoutes'
// Game publish (backend hosting): POST /api/publish writes the multi-file web
// export, GET /published/ serves it for instant play. The static-hosting
// publish path is client-only and never hits these routes.
import { registerPublishRoutes } from './server/publishRoute'

// ──────────────────────────────────────────────────────────────
// Pokered data routes — registered BEFORE the AI plugin so the middleware
// order stays byte-identical to the old inline plugins (gfx write → gfx
// static → wasm → maps/blocksets/tilesets → trainers → ui-layouts → pokemon →
// moves → items → categories/shops). All roots derive from the project root
// in server/api/projectConfig.ts, re-evaluated per request.
// ──────────────────────────────────────────────────────────────
function pokeredRoutesPlugin(): Plugin {
  return {
    name: 'pokered-editor-data-api',
    configureServer(server: ViteDevServer) {
      registerPokeredRoutes(server)
      registerPublishRoutes(server)
    },
  }
}

// ──────────────────────────────────────────────────────────────
// AI assistant + sprite generation pipeline (ported from dotzuki-editor; the
// routes live under server/api/routes/). The project root defaults to the
// workspace root — see server/api/projectConfig.ts.
// Registered LAST in the plugins array so the pokered-specific routes above
// keep their exact paths; every handler here sits on a distinct prefix
// (/api/ai/*, /api/sprites/*, /api/project/*, /api/scene-lint,
// /api/editor-settings) and none of them overlap the routes above.
// ──────────────────────────────────────────────────────────────
function apiAiPlugin(): Plugin {
  return {
    name: 'pokered-editor-ai-api',
    configureServer(server: ViteDevServer) {
      // Register the built-in AI actions (generate-scene, generate-gui,
      // generate-data, …) so /api/ai/run + the legacy shims can resolve them.
      registerBuiltinActions()

      // ── CORS — matches all /api/* and falls through (same as dotzuki-editor). ──
      server.middlewares.use('/api', (req, res, next) => {
        res.setHeader('Access-Control-Allow-Origin', '*')
        res.setHeader('Access-Control-Allow-Methods', 'GET,PUT,POST,DELETE,OPTIONS')
        res.setHeader('Access-Control-Allow-Headers', 'Content-Type')
        if (req.method === 'OPTIONS') {
          res.writeHead(204); res.end(); return
        }
        next()
      })

      registerProject(server)
      registerAi(server)
      // Sprite routes: /api/sprites/* (presets, animated sheet pipeline) and
      // /api/sprites/generate-single — the Pixel activity's one-shot static
      // sprite generation (see server/spriteSingle.ts).
      registerSprites(server)
    },
  }
}

export default defineConfig({
  // For GitHub Pages deployment (sub-path /<repo>/editor/), set
  // VITE_BASE_PATH=/<repo>/editor/ — mirrors crates/pokered-web/frontend.
  base: process.env.VITE_BASE_PATH || '/',
  plugins: [
    vue(),
    tailwindcss(),
    pokeredRoutesPlugin(),
    // AI assistant + sprite pipeline (defined above) — must stay last.
    apiAiPlugin(),
  ],
})
