// Placeholder frontend entry point. StoryBloom Studio's real UI (framework
// choice, component structure, state management) is intentionally not
// implemented yet - this file only exists so `tauri.conf.json`'s
// `frontendDist` points at something valid and the dev server has an
// entry module to load.
//
// Once a frontend framework is chosen, this `frontend/` directory is
// expected to be replaced with that framework's scaffold (e.g. a Vite
// project), and `tauri.conf.json`'s `build.beforeDevCommand` /
// `beforeBuildCommand` / `frontendDist` updated to match.

import { invoke } from "@tauri-apps/api/core";

async function checkBackendBridge() {
  try {
    // `ping` is the scaffolding health-check command defined in
    // `src-tauri/src/commands/mod.rs`.
    const response = await invoke("ping");
    console.log("Rust backend responded:", response);
  } catch (err) {
    console.error("Failed to reach Rust backend:", err);
  }
}

window.addEventListener("DOMContentLoaded", checkBackendBridge);
