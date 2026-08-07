# StoryBloom Studio

A cross-platform desktop app built with **Rust + Tauri**, structured around
**MVVM**:

- **Model** – `crates/storybloom-core` (domain models + services) and
  `crates/storybloom-db` (SQLite persistence).
- **ViewModel** – `src-tauri/src/viewmodels` (translates between commands
  and core services).
- **View** – `src-tauri` (Tauri shell / commands) + `frontend/` (WebView UI;
  framework not chosen yet).

This repository currently contains **structure only, plus one implemented
feature: the Story Engine** (see below). No UI is wired up yet - it's meant
to compile, launch an empty window, log to stdout and a rotating file,
connect to SQLite with zero-migration scaffolding in place, and generate
stories via OpenAI when called directly (Rust-only, no command/UI surface
yet).

## Layout

```
storybloom-studio/
├── Cargo.toml                  # workspace root
├── rust-toolchain.toml         # pinned toolchain
├── rustfmt.toml
├── config/                     # layered TOML configuration
│   ├── default.toml
│   ├── development.toml
│   ├── production.toml
│   └── local.toml.example      # copy -> local.toml for gitignored overrides
├── crates/
│   ├── storybloom-core/        # Model: domain types + business logic
│   ├── storybloom-config/      # Settings loading (layered TOML + env)
│   └── storybloom-db/          # SQLite connection pool + migrations
├── src-tauri/                  # Tauri shell (View entry point)
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── icons/
│   └── src/
│       ├── main.rs             # binary entry point
│       ├── app.rs              # startup wiring: config -> logging -> db -> Tauri
│       ├── paths.rs            # cross-platform config/data/log dir resolution
│       ├── state.rs            # AppState (managed Tauri state)
│       ├── logging.rs          # tracing subscriber setup
│       ├── commands/           # #[tauri::command] entry points
│       └── viewmodels/         # ViewModel layer
└── frontend/                   # View (WebView UI) - placeholder only
```

## Configuration

Settings are layered (see `crates/storybloom-config/src/lib.rs`):

1. `config/default.toml`
2. `config/{STORYBLOOM_ENV}.toml` (defaults to `development`)
3. `config/local.toml` (gitignored, optional)
4. `STORYBLOOM__*` environment variables (double-underscore delimited, e.g.
   `STORYBLOOM__DATABASE__MAX_CONNECTIONS=10`)

In production builds, `config/*.toml` is bundled as a Tauri resource
alongside the executable (see `bundle.resources` in
`src-tauri/tauri.conf.json`) and resolved relative to the running binary;
see `src-tauri/src/paths.rs` for the full resolution order.

## Data & logs

Resolved via the `directories` crate to the platform-appropriate app-data
directory (e.g. `~/.local/share/storybloom-studio` on Linux, `~/Library/
Application Support/...` on macOS, `%APPDATA%\...` on Windows):

- SQLite database: `<data_dir>/<database.file_name>`
- Logs: `<data_dir>/logs/storybloom.log.<date>` (daily rotation)

## Story Engine

`storybloom-core::services::story_engine` calls an OpenAI-compatible chat
completion API with strict structured outputs, so the model's response
deserializes directly into a strongly typed [`StoryResult`] with no manual
text parsing:

```rust
use storybloom_core::{StoryDuration, StoryEngine, StoryEngineConfig, StoryRequest, StoryType, Tone};

let engine = StoryEngine::new(StoryEngineConfig {
    base_url: "https://api.openai.com/v1".into(),
    api_key: std::env::var("OPENAI_API_KEY")?,
    model: "gpt-4o-mini".into(),
    temperature: 0.9,
    max_output_tokens: 800,
    request_timeout: std::time::Duration::from_secs(30),
})?;

let request = StoryRequest::new(StoryType::Adventure, StoryDuration::Short, Tone::Lighthearted);
let result = engine.generate(&request).await?; // -> StoryResult { title, story, description, hashtags }
```

Try it standalone:

```sh
OPENAI_API_KEY=sk-... cargo run -p storybloom-core --example generate_story
```

Types:

- `StoryType`, `Tone` - enums with common presets plus a `Custom(String)`
  escape hatch.
- `StoryDuration` - `Short` / `Medium` / `Long` presets; maps to a target
  word count internally so the prompt gets concrete guidance.
- `Hashtag` - validated, `#`-normalized newtype (`Vec<Hashtag>` in the
  result, never raw `Vec<String>`).
- `StoryResult` - `{ title, story, description, hashtags }`, deserialized
  straight from the model's structured JSON response.

Configuration lives under `[ai]` in `config/*.toml` (provider, base URL,
model, temperature, token limit, timeout). The API key is deliberately
**not** committed - set it via `STORYBLOOM__AI__API_KEY`, a plain
`OPENAI_API_KEY` env var (checked as a fallback), or gitignored
`config/local.toml`. In the Tauri app, a missing key doesn't block
startup: `AppState.view_models.story_engine` is simply `None` until one is
configured (see `src-tauri/src/state.rs`).



```sh
cargo check --workspace          # compile-check everything
cd src-tauri && cargo tauri dev  # launch the app (requires the Tauri CLI)
```

No `cargo tauri` CLI config for a specific frontend build step is set up
yet since `frontend/` has no real framework — `beforeDevCommand` /
`beforeBuildCommand` are intentionally omitted from `tauri.conf.json` until
one is chosen.

## Adding a feature (once this scaffolding is used for real)

1. Domain types + business logic → `storybloom-core::models` /
   `storybloom-core::services`.
2. Schema changes → a new file in `crates/storybloom-db/src/migrations/`.
3. Bridge logic → a view-model in `src-tauri/src/viewmodels/`.
4. Expose it → a `#[tauri::command]` in `src-tauri/src/commands/`,
   registered in the `tauri::generate_handler![...]` list in
   `src-tauri/src/app.rs`.
