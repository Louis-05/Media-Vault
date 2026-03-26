# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Desktop app for searching media (images, videos, GIFs, audio) by text descriptions using local EmbeddingGemma300M sentence embeddings. Built with Tauri v2 (Rust backend) + Svelte 5 (frontend). All processing runs locally — no cloud APIs.

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
- **Custom URI scheme**: `media://localhost/<path>` serves media files with Range request support for video seeking (registered in `lib.rs`). On Windows, the frontend uses `http://media.localhost/<path>` (see `toMediaUrl` in `api.ts`).

### Backend modules (`src-tauri/src/`)

| Module | Purpose |
|---|---|
| `lib.rs` | Tauri builder, plugin registration, command handler list, `media://` URI scheme |
| `state.rs` | `AppState` — holds `Mutex<Option<T>>` for db, vault_path, embedder, loading_status, worker channels |
| `commands/` | IPC command handlers: `vault.rs`, `media.rs`, `search.rs`, `descriptions.rs`, `tags.rs` |
| `db/` | SQLite via rusqlite (bundled): `schema.rs` (DDL + migrations), `models.rs` (serde structs), `queries.rs` (all SQL) |
| `embedding/mod.rs` | fastembed `EmbeddingGemma300M` (text-only). ~300MB model auto-downloads on first use |
| `tags_file.rs` | Read/write `tags.json` in vault root (maps media_id → {tag_key → [values]}). Falls back to legacy `descriptions.json` |
| `media/mod.rs` | File import, UUID rename, media type detection, BLAKE3 checksum deduplication |
| `thumbnail/mod.rs` | ffmpeg-based preview generation: WebP thumbnails + video/GIF previews in `previews/` |
| `worker/mod.rs` | Background processing thread: generates thumbnails, previews, and embeddings for unprocessed media. Supports pause/resume and yields to search requests |
| `watcher/mod.rs` | Vault refresh: cleanup stale entries, auto-import new files, sync tags from `tags.json`, generate missing previews and embeddings |
| `logging.rs` | `flexi_logger` init (console with colors) |

### Frontend structure (`src/`)

- `App.svelte` — page router (loading → folder picker → search, descriptions, tags, or settings)
- `lib/api.ts` — typed wrappers around all Tauri invoke calls
- `lib/stores/vault.ts` — Svelte stores (currentVault, mediaList, currentPage, etc.)
- `lib/components/` — UI components (MediaCard, SearchPage, DescriptionsPage, TagsPage, SettingsPage, etc.)

## Key Concepts

- **Vault**: A folder containing `vault.db` (SQLite), `tags.json`, `media/` subfolder, and `previews/` subfolder. All paths are relative for portability.
- **Media files** are renamed to `{uuid}.{ext}` on import. Original filename stored in DB. Deduplication via BLAKE3 checksum.
- **Text-only embeddings**: The app uses `EmbeddingGemma300M` (fastembed, text-only). Descriptions are embedded with document prompt format (`title: none | text: {desc}`), search queries with query prompt format (`task: search result | query: {text}`). Cosine similarity ranks results.
- **Tags system**: Media items can have multiple tags organized by key (e.g., `description`, `media_type`, custom keys). Tags are dual-stored: SQLite (`tag_keys` + `tags` tables) and `tags.json` (portable backup). On vault open, `tags.json` is the canonical source.
- **Worker thread**: Background processing runs in a dedicated thread (`worker/mod.rs`) communicating via `mpsc` channels. Supports `Wake`/`Stop`/`Pause`/`Resume` commands. Yields to search requests so embedding computation doesn't block searches.
- **Processed flag**: Each media row has a `processed` column (0 = needs processing, 1 = done). The worker picks unprocessed items and runs them through the pipeline: thumbnail → preview → description sync → text embedding → mark processed.

## Embeddings Table Schema

```sql
PRIMARY KEY (media_id, embedding_type)  -- composite key
```

Each media item can have one `'text'` row. The `model_name` column tracks which model generated each embedding (enables migration detection).

## Event System

Backend emits Tauri events that the frontend listens to:
- `loading-status` — embedding model load progress
- `media-changed` — after imports, deletions, or vault refresh (frontend reloads media list)
- `duplicates-skipped` — when import skips files already in vault (by checksum)

## Lock Ordering & Search Priority

When acquiring multiple mutexes on `AppState`, always lock in this order: `embedder` → `db`. Never hold both simultaneously if possible — compute embedding first, then write to DB.

The worker thread yields to pending search operations via a two-channel handshake: `search_request_tx` signals "search wants the embedder", `search_done_tx` signals "search is finished". This prevents long embedding runs from blocking user searches.

## DB Schema & Migrations

`schema.rs` uses `CREATE TABLE IF NOT EXISTS` for initial setup and detects missing columns via `SELECT col FROM table LIMIT 0` probes for migrations. Key tables:
- `media` — id, extension, media_type, codec, description, checksum, processed, duration, etc.
- `embeddings` — composite PK (media_id, embedding_type), vector BLOB, model_name
- `tag_keys` — registry of tag key names (e.g., `description`, `media_type`)
- `tags` — composite PK (media_id, key, value), references tag_keys with `ON UPDATE CASCADE ON DELETE CASCADE`

## Build Details

- Rust edition 2021, crate name `media-vault`, lib name `media_vault_lib`
- Frontend dev server: port 1420
- Tauri identifier: `com.media-vault.desktop`
- Preferences (`preferences.json` next to executable): last opened vault path, zoom level
- Release profile: LTO enabled, single codegen unit, stripped symbols, panic=abort

## Common Tasks

- **Add a Tauri command**: Define in `src-tauri/src/commands/`, register in `lib.rs` `generate_handler![]`, add TypeScript wrapper in `src/lib/api.ts`.
- **Modify DB schema**: Update `src-tauri/src/db/schema.rs` (uses `CREATE TABLE IF NOT EXISTS`, migrations detect missing columns/tables via SELECT probes).
- **Add a media type**: Update the extension constants in `src-tauri/src/media/mod.rs`.
- **Add a tag key**: Insert into `tag_keys` table. The `description` and `media_type` keys are created automatically by the schema.
