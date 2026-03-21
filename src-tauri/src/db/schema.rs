use rusqlite::Connection;

pub fn initialize_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS media (
            id            TEXT PRIMARY KEY,
            extension     TEXT NOT NULL,
            media_type    TEXT NOT NULL,
            codec         TEXT,
            description   TEXT,
            imported_at   TEXT NOT NULL DEFAULT (datetime('now')),
            file_size     INTEGER,
            width         INTEGER,
            height        INTEGER,
            checksum      TEXT
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_media_checksum ON media(checksum) WHERE checksum IS NOT NULL;

        CREATE TABLE IF NOT EXISTS embeddings (
            media_id   TEXT PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
            vector     BLOB NOT NULL,
            model_name TEXT NOT NULL DEFAULT 'EmbeddingGemma300M',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_media_no_desc ON media(id) WHERE description IS NULL;

        PRAGMA foreign_keys = ON;
        ",
    )?;

    // Migration: add checksum column to existing databases
    let has_checksum: bool = conn
        .prepare("SELECT checksum FROM media LIMIT 0")
        .is_ok();
    if !has_checksum {
        conn.execute_batch("ALTER TABLE media ADD COLUMN checksum TEXT;")?;
    }

    Ok(())
}
