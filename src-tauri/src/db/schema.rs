use rusqlite::Connection;

pub fn initialize_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;

        CREATE TABLE IF NOT EXISTS media (
            id            TEXT PRIMARY KEY,
            extension     TEXT NOT NULL,
            media_type    TEXT NOT NULL,
            codec         TEXT,
            imported_at   TEXT NOT NULL DEFAULT (datetime('now')),
            file_size     INTEGER,
            width         INTEGER,
            height        INTEGER,
            checksum      TEXT,
            processed     INTEGER NOT NULL DEFAULT 0,
            duration      REAL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_media_checksum ON media(checksum) WHERE checksum IS NOT NULL;

        PRAGMA foreign_keys = ON;
        ",
    )?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS embeddings (
            media_id       TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
            embedding_type TEXT NOT NULL DEFAULT 'text',
            vector         BLOB NOT NULL,
            model_name     TEXT NOT NULL DEFAULT 'Qwen3VLEmbedding2B',
            created_at     TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (media_id, embedding_type)
        );
        ",
    )?;

    // Tag system: tag_keys registry + tags table
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tag_keys (
            key        TEXT PRIMARY KEY,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tags (
            media_id  TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
            key       TEXT NOT NULL REFERENCES tag_keys(key) ON UPDATE CASCADE ON DELETE CASCADE,
            value     TEXT NOT NULL,
            PRIMARY KEY (media_id, key, value)
        );

        CREATE INDEX IF NOT EXISTS idx_tags_key ON tags(key);

        INSERT OR IGNORE INTO tag_keys (key) VALUES ('description');
        INSERT OR IGNORE INTO tag_keys (key) VALUES ('media_type');
        ",
    )?;

    Ok(())
}
