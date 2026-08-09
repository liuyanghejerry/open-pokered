# build/ — electron-builder buildResources

electron-builder reads this folder (`directories.buildResources`) for signing
and branding assets. It is **not** bundled into the app — it only feeds the
packaging step.

| File | Purpose | Status |
|------|---------|--------|
| `entitlements.mac.plist` | Hardened-runtime entitlements applied when signing for notarization (JIT / unsigned memory / library validation). | ✅ committed |
| `icon.icns` | macOS app icon (1024×1024 source recommended). | ⬜ drop in to replace the default Electron icon |
| `icon.png` | Linux app icon (512×512+). | ⬜ optional |
| `icon.ico` | Windows app icon. | ⬜ optional |

Signing/notarization is off by default and turns on via env vars — see the
header of `../electron-builder.cjs` for the exact variables.

To generate `icon.icns` from a 1024×1024 PNG:

```bash
mkdir icon.iconset
sips -z 16 16   icon-1024.png --out icon.iconset/icon_16x16.png
sips -z 32 32   icon-1024.png --out icon.iconset/icon_16x16@2x.png
sips -z 128 128 icon-1024.png --out icon.iconset/icon_128x128.png
sips -z 256 256 icon-1024.png --out icon.iconset/icon_256x256.png
sips -z 512 512 icon-1024.png --out icon.iconset/icon_512x512.png
cp icon-1024.png icon.iconset/icon_512x512@2x.png
iconutil -c icns icon.iconset -o icon.icns
rm -rf icon.iconset
```
