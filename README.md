# Media Vault

> **Note:** This project was built for my personal use. There are no plans to maintain it for a wider audience, and it may break, change, or be abandoned at any time. Use at your own risk.

A desktop application for searching media files (images, videos, GIFs) by text descriptions using locally-run sentence embeddings.

Describe your media, then find them instantly by searching in natural language. All processing runs locally — no cloud APIs required.

## Features

- **Semantic search** — find media by meaning, not just keywords, using Gemma embedding models
- **Drag and drop** — import media by dropping files into the window, drag them back out to use them
- **Portable vaults** — each vault is a self-contained folder (SQLite DB + media files) that can be moved or copied anywhere
- **Auto-import** — files copied directly into the media folder are detected and imported automatically
- **Local processing** — embeddings are computed on-device with no external dependencies beyond ffmpeg

## Prerequisites

### System dependencies

**Linux (Debian/Ubuntu):**

```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev ffmpeg
```

**Linux (Fedora):**

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel ffmpeg
```

**macOS:**

```bash
brew install ffmpeg
```

**Windows:**

Download and install [ffmpeg](https://ffmpeg.org/download.html) and ensure it is in your PATH.

### Toolchains

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v18+)
- Tauri CLI: `cargo install tauri-cli --version "^2"`

## Building

```bash
# Install frontend dependencies
npm install

# Development (hot-reload frontend, debug Rust backend)
cargo tauri dev

# Production build
cargo tauri build
```

The production binary and platform-specific packages (.deb, .rpm, .AppImage, .dmg, .msi) are output to `src-tauri/target/release/bundle/`.

## Usage

1. Launch the app and click **Open or Create Vault** to select a folder
2. Import media by dragging files into the window
3. Go to **Descriptions** to describe each media file
4. Search by typing a description in the search bar — results are ranked by similarity
5. Drag media out of the window to copy them to another application

Files can also be copied directly into the vault's `media/` subfolder — the app detects and imports them automatically.

### Vault structure

```
my-vault/
  vault.db          # SQLite database (metadata, embeddings, thumbnails)
  media/            # All media files, renamed to UUID
```

The vault folder is fully portable. Move or copy it to another machine and open it in the app.

## First-run note

The first time you save a description or perform a search, the Gemma embedding model (~600MB) is downloaded and cached automatically. Subsequent launches use the cached model.
