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
            description   TEXT,
            imported_at   TEXT NOT NULL DEFAULT (datetime('now')),
            file_size     INTEGER,
            width         INTEGER,
            height        INTEGER,
            checksum      TEXT,
            processed     INTEGER NOT NULL DEFAULT 0,
            duration      REAL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_media_checksum ON media(checksum) WHERE checksum IS NOT NULL;

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

    // Migration: add processed column to existing databases (default 1 = already processed)
    let has_processed: bool = conn
        .prepare("SELECT processed FROM media LIMIT 0")
        .is_ok();
    if !has_processed {
        conn.execute_batch("ALTER TABLE media ADD COLUMN processed INTEGER NOT NULL DEFAULT 1;")?;
    }

    // Migration: add duration column to existing databases
    let has_duration: bool = conn
        .prepare("SELECT duration FROM media LIMIT 0")
        .is_ok();
    if !has_duration {
        conn.execute_batch("ALTER TABLE media ADD COLUMN duration REAL;")?;
    }

    // Migration: recreate embeddings table with composite PK (media_id, embedding_type)
    // Old schema had media_id as sole PRIMARY KEY. New schema has (media_id, embedding_type).
    let has_embedding_type: bool = conn
        .prepare("SELECT embedding_type FROM embeddings LIMIT 0")
        .is_ok();

    if !has_embedding_type {
        // Old table exists without embedding_type column — migrate
        let old_table_exists: bool = conn
            .prepare("SELECT media_id FROM embeddings LIMIT 0")
            .is_ok();

        if old_table_exists {
            conn.execute_batch(
                "
                ALTER TABLE embeddings RENAME TO embeddings_old;

                CREATE TABLE embeddings (
                    media_id       TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
                    embedding_type TEXT NOT NULL DEFAULT 'text',
                    vector         BLOB NOT NULL,
                    model_name     TEXT NOT NULL DEFAULT 'Qwen3VLEmbedding2B',
                    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
                    PRIMARY KEY (media_id, embedding_type)
                );

                INSERT INTO embeddings (media_id, embedding_type, vector, model_name, created_at)
                    SELECT media_id, 'text', vector, model_name, created_at FROM embeddings_old;

                DROP TABLE embeddings_old;
                ",
            )?;
        } else {
            // Fresh database — create the new schema directly
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
        }
    } else {
        // Table already has embedding_type — ensure it exists
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
    }

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

    // Migration: copy existing media.description values into the tags table
    let has_description_col: bool = conn
        .prepare("SELECT description FROM media LIMIT 0")
        .is_ok();
    if has_description_col {
        conn.execute_batch(
            "
            INSERT OR IGNORE INTO tags (media_id, key, value)
                SELECT id, 'description', description FROM media
                WHERE description IS NOT NULL AND description != '';
            ",
        )?;
    }

    Ok(())
}
