// ──────────────────────────────────────────────────────────────────────────
// electron-builder configuration (pokered-editor).
//
// DEFAULT (no certificate, no Apple credentials) → an ad-hoc / unsigned build:
// it runs on this machine and on copies transferred without the download
// "quarantine" flag (USB, scp, git). Nothing below forces signing or
// notarization, so `pnpm electron:pack` / `electron:dist` keep working with
// zero setup.
//
// GATEKEEPER-CLEAN (double-click-to-open after download) requires a PAID Apple
// Developer Program account. Once you have it, set the env vars below and this
// config switches ON Developer ID signing + Apple notarization automatically —
// no code edits, just fill the blanks:
//
//   ── 1. Signing — a "Developer ID Application" certificate ──
//     Option A (recommended, CI-friendly): export the cert as a .p12 and point to it
//        export CSC_LINK=/absolute/path/DeveloperID.p12     # or a base64 of the .p12
//        export CSC_KEY_PASSWORD='the .p12 password'
//     Option B: use a cert already in your login keychain, by its exact name
//        export CSC_NAME="Developer ID Application: Your Name (TEAMID)"
//
//   ── 2. Notarization — Apple notary service (needs signing above) ──
//        export APPLE_TEAM_ID='YOURTEAMID'                  # 10-char team id
//     then EITHER an Apple ID + app-specific password (appleid.apple.com → Sign-In & Security)
//        export APPLE_ID='you@example.com'
//        export APPLE_APP_SPECIFIC_PASSWORD='xxxx-xxxx-xxxx-xxxx'
//     OR an App Store Connect API key (.p8)
//        export APPLE_API_KEY=/absolute/path/AuthKey_XXXXXXXXXX.p8
//        export APPLE_API_KEY_ID='XXXXXXXXXX'
//        export APPLE_API_ISSUER='xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx'
//
// App icon: drop build/icon.icns (and build/icon.png / icon.ico for other OSes)
// and electron-builder picks it up automatically. See build/README.md.
// ──────────────────────────────────────────────────────────────────────────

// Sign only when a certificate source is explicitly provided, so a plain local
// build never trips over a missing cert.
const hasSigningCert = Boolean(process.env.CSC_LINK || process.env.CSC_NAME)

// Notarize only when signing AND full notary credentials are present, otherwise
// every local `electron:dist` would try (and fail) to reach Apple's servers.
const teamId = process.env.APPLE_TEAM_ID
const hasNotaryCreds = Boolean(
  teamId &&
    ((process.env.APPLE_ID && process.env.APPLE_APP_SPECIFIC_PASSWORD) ||
      (process.env.APPLE_API_KEY && process.env.APPLE_API_KEY_ID && process.env.APPLE_API_ISSUER)),
)
const willNotarize = hasSigningCert && hasNotaryCreds

if (process.env.CSC_LINK || process.env.CSC_NAME || teamId) {
  console.log(
    `[electron-builder] macOS signing=${hasSigningCert ? 'ON' : 'off'} ` +
      `notarize=${willNotarize ? 'ON' : 'off'}`,
  )
}

/** @type {import('electron-builder').Configuration} */
module.exports = {
  appId: 'com.pokered.editor',
  productName: 'Pokered Editor',
  // buildResources (default: build/) holds the icon + entitlements; output is
  // the finished artifacts (gitignored).
  directories: { output: 'release', buildResources: 'build' },
  files: [
    'dist/**/*',
    'dist-electron/**/*',
    '!dist-electron/wasm-pkg/**',
    'electron/**/*',
    'package.json',
  ],
  // The WASM layout-preview pkg rides alongside the app (Resources/wasm-pkg),
  // not inside the asar — the /wasm route reads it via DOTZUKI_WASM_ROOT.
  extraResources: [{ from: 'dist-electron/wasm-pkg', to: 'wasm-pkg' }],
  asar: true,
  mac: {
    category: 'public.app-category.developer-tools',
    target: ['dmg', 'zip'],
    // Hardened runtime + entitlements are a notarization requirement, but they
    // change how the app launches, so enable them ONLY for real signed builds —
    // the default unsigned build stays byte-for-byte the behavior we verified.
    hardenedRuntime: hasSigningCert,
    entitlements: 'build/entitlements.mac.plist',
    entitlementsInherit: 'build/entitlements.mac.plist',
    gatekeeperAssess: false,
    // `identity` is intentionally left unset: with a cert/env present
    // electron-builder signs with it; without one it skips Developer ID signing
    // (ad-hoc). Set CSC_NAME/CSC_LINK to sign.
    notarize: willNotarize ? { teamId } : false,
  },
  dmg: {
    // Standard drag-to-Applications layout.
    contents: [
      { x: 130, y: 220 },
      { x: 410, y: 220, type: 'link', path: '/Applications' },
    ],
  },
  win: {
    // Windows Authenticode signing is separate: set WIN_CSC_LINK +
    // WIN_CSC_KEY_PASSWORD (a .pfx) to sign; unset → unsigned, SmartScreen may warn.
    target: ['nsis'],
  },
  linux: {
    target: ['AppImage'],
    category: 'Development',
  },
}
