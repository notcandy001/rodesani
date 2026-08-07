# Icons

Placeholder icon set (a simple generated mark), committed so the project
builds out of the box - `tauri::generate_context!()` requires these files
to exist at compile time, it doesn't just skip missing ones.

Contains: `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`,
`icon.ico` (multi-resolution Windows icon), `icon.icns` (macOS icon).

Once real artwork exists, regenerate the whole set from a single source
image with:

    cargo tauri icon path/to/source-icon.png
