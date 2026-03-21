# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Desktop app for searching media (images, videos, GIFs, audio) by text descriptions using local Gemma sentence embeddings. Built with Tauri v2 (Rust backend) + Svelte 5 (frontend).

## Build & Run

```bash
npm install                # frontend dependencies (first time)
cargo tauri dev            # development mode (Vite dev server + Tauri)
cargo tauri build          # production build → src-tauri/target/release/bundle/
```

**Prerequisites:** Rust toolchain, Node.js 18+, Tauri CLI (`cargo install tauri-cli --version "^2"`), `ffmpeg` in PATH, and platform-specific libs:
- **Linux (Debian/Ubuntu):** `libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`
- **Linux (Fedora):** `gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel`
- **macOS:** `brew install ffmpeg`
- **Windows:** ffmpeg in PATH

No tests or linter configured yet.

## Architecture

- **Frontend** (`src/`): Svelte 5 + TypeScript + Vite. Only manages the GUI — no business logic.
- **Backend** (`src-tauri/src/`): Rust. All business logic lives here.
- **IPC**: Frontend calls Rust `#[tauri::command]` functions via `@tauri-apps/api` invoke. Commands are in `src-tauri/src/commands/`, TypeScript wrappers mirror them in `src/lib/api.ts`.
- **Custom URI scheme**: `media://localhost/<path>` serves media files with Range request support for video seeking (registered in `lib.rs`).

### Backend modules (`src-tauri/src/`)

| Module | Purpose |
|---|---|
| `lib.rs` | Tauri builder, plugin registration, command handler list, `media://` URI scheme |
| `state.rs` | `AppState` — holds `Mutex<Option<T>>` for db, vault_path, embedder, watcher, loading_status |
| `commands/` | IPC command handlers: `vault.rs`, `media.rs`, `search.rs`, `descriptions.rs` |
| `db/` | SQLite via rusqlite (bundled): `schema.rs` (DDL), `models.rs` (serde structs), `queries.rs` (all SQL) |
| `embedding/mod.rs` | fastembed `EmbeddingGemma300M` (768-dim, ONNX). ~600MB model auto-downloads on first use |
| `descriptions_file.rs` | Read/write `descriptions.json` in vault root (maps media_id → description text) |
| `media/mod.rs` | File import, UUID rename, media type detection, BLAKE3 checksum deduplication |
| `thumbnail/mod.rs` | ffmpeg-based preview generation: WebP thumbnails + video/GIF previews, stored as files in `previews/` |
| `watcher/mod.rs` | Background vault refresh: cleanup stale entries, auto-import new files, sync descriptions, generate missing previews |

### Frontend structure (`src/`)

- `App.svelte` — page router (loading → folder picker → search or descriptions)
- `lib/api.ts` — typed wrappers around all Tauri invoke calls
- `lib/stores/vault.ts` — Svelte stores (currentVault, mediaList, currentPage, etc.)
- `lib/components/` — UI components (MediaCard, MediaGrid, SearchBar, etc.)

## Key Concepts

- **Vault**: A folder containing `vault.db` (SQLite), `descriptions.json`, `media/` subfolder, and `previews/` subfolder. All paths are relative for portability.
- **Media files** are renamed to `{uuid}.{ext}` on import. Original filename stored in DB. Deduplication via BLAKE3 checksum.
- **Descriptions** are dual-stored: SQLite (for queries/embeddings) and `descriptions.json` (portable text backup). On vault open, `descriptions.json` is the canonical source — it syncs into the DB and recomputes embeddings for any changes.
- **Search** is brute-force cosine similarity over all stored 768-dim vectors. No vector index.
- **Embedding model** loads in a background thread at app startup (`lib.rs` setup hook). Frontend polls/listens for `loading-status` events.
- **Embedding prompts**: Documents use `"title: none | text: {description}"`, queries use `"task: search result | query: {query}"` (fastembed Gemma format).

## Event System

Backend emits Tauri events that the frontend listens to:
- `loading-status` — embedding model load progress
- `media-changed` — after imports, deletions, or vault refresh (frontend reloads media list)
- `duplicates-skipped` — when import skips files already in vault (by checksum)

## Lock Ordering

When acquiring multiple mutexes on `AppState`, always lock in this order: `embedder` → `db`. Never hold both simultaneously if possible — compute embedding first, then write to DB.

## Vault Refresh Pipeline

On vault open (and manual refresh), `watcher/mod.rs` runs these steps in order:
1. **Cleanup** — remove DB entries for files no longer on disk, delete orphaned preview files
2. **Process new files** — scan `media/` folder, import any unregistered files (rename non-UUIDs)
3. **Sync descriptions** — read `descriptions.json`, update DB, recompute embeddings for changes
4. **Generate previews** — create missing thumbnails and video/GIF previews

## Build Details

- Rust edition 2021, crate name `media-vault`, lib name `media_vault_lib`
- Frontend dev server: port 1420
- Tauri identifier: `com.media-vault.desktop`
- Preferences (`preferences.json` next to executable): last opened vault path, zoom level

## Common Tasks

- **Add a Tauri command**: Define in `src-tauri/src/commands/`, register in `lib.rs` `generate_handler![]`, add TypeScript wrapper in `src/lib/api.ts`.
- **Modify DB schema**: Update `src-tauri/src/db/schema.rs` (uses `CREATE TABLE IF NOT EXISTS`, backward compat via `ALTER TABLE`).
- **Add a media type**: Update the extension constants in `src-tauri/src/media/mod.rs`.
