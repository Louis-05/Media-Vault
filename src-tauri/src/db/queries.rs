use rusqlite::{params, Connection, OptionalExtension};

use super::models::{DescriptionPageData, MediaInfo, TagInfo, TagKeyInfo, VaultInfo};

pub fn get_vault_info(conn: &Connection, path: &str) -> rusqlite::Result<VaultInfo> {
    let count: u32 =
        conn.query_row("SELECT COUNT(*) FROM media WHERE processed = 1", [], |row| row.get(0))?;
    Ok(VaultInfo {
        path: path.to_string(),
        media_count: count,
    })
}

pub fn insert_media(
    conn: &Connection,
    id: &str,
    extension: &str,
    media_type: &str,
    codec: Option<&str>,
    file_size: Option<u64>,
    checksum: Option<&str>,
    duration: Option<f64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO media (id, extension, media_type, codec, file_size, checksum, processed, duration) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![id, extension, media_type, codec, file_size.map(|s| s as i64), checksum, duration],
    )?;
    // Auto-add media_type tag
    conn.execute(
        "INSERT OR IGNORE INTO tags (media_id, key, value) VALUES (?1, 'media_type', ?2)",
        params![id, media_type],
    )?;
    Ok(())
}

pub fn find_media_by_checksum(conn: &Connection, checksum: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT id FROM media WHERE checksum = ?1",
        params![checksum],
        |row| row.get(0),
    )
    .optional()
}

pub fn get_media_list(
    conn: &Connection,
    offset: u32,
    limit: u32,
) -> rusqlite::Result<Vec<MediaInfo>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.extension, m.media_type, m.codec, t.value, m.duration
         FROM media m
         LEFT JOIN tags t ON m.id = t.media_id AND t.key = 'description'
         WHERE m.processed = 1
         ORDER BY m.imported_at DESC LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        let description: Option<String> = row.get(4)?;
        Ok(MediaInfo {
            id: row.get(0)?,
            extension: row.get(1)?,
            media_type: row.get(2)?,
            codec: row.get(3)?,
            has_description: description.is_some(),
            description,
            duration: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_media_by_id(conn: &Connection, media_id: &str) -> rusqlite::Result<Option<MediaInfo>> {
    conn.query_row(
        "SELECT m.id, m.extension, m.media_type, m.codec, t.value, m.duration
         FROM media m
         LEFT JOIN tags t ON m.id = t.media_id AND t.key = 'description'
         WHERE m.id = ?1",
        params![media_id],
        |row| {
            let description: Option<String> = row.get(4)?;
            Ok(MediaInfo {
                id: row.get(0)?,
                extension: row.get(1)?,
                media_type: row.get(2)?,
                codec: row.get(3)?,
                has_description: description.is_some(),
                description,
                duration: row.get(5)?,
            })
        },
    )
    .optional()
}

pub fn delete_media(conn: &Connection, media_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM media WHERE id = ?1", params![media_id])?;
    Ok(())
}

pub fn get_filtered_media_ids(
    conn: &Connection,
    filter_missing_desc: bool,
    filter_missing_tags: bool,
) -> rusqlite::Result<Vec<String>> {
    let mut conditions = vec!["m.processed = 1".to_string()];
    if filter_missing_desc {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key = 'description')"
                .to_string(),
        );
    }
    if filter_missing_tags {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key NOT IN ('description', 'media_type'))"
                .to_string(),
        );
    }
    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT m.id FROM media m WHERE {where_clause} ORDER BY m.imported_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

pub fn get_media_for_description(
    conn: &Connection,
    index: u32,
    filter_missing_desc: bool,
    filter_missing_tags: bool,
) -> rusqlite::Result<Option<DescriptionPageData>> {
    // Total is always all processed media
    let total_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM media WHERE processed = 1",
        [],
        |row| row.get(0),
    )?;
    if total_count == 0 {
        return Ok(None);
    }

    // Build filtered list query based on flags
    let mut conditions = vec!["m.processed = 1".to_string()];
    if filter_missing_desc {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key = 'description')"
                .to_string(),
        );
    }
    if filter_missing_tags {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key NOT IN ('description', 'media_type'))"
                .to_string(),
        );
    }
    let where_clause = conditions.join(" AND ");

    let filtered_count: u32 = conn.query_row(
        &format!("SELECT COUNT(*) FROM media m WHERE {where_clause}"),
        [],
        |row| row.get(0),
    )?;
    if filtered_count == 0 {
        return Ok(None);
    }

    let actual_index = index.min(filtered_count - 1);

    let list_sql = format!(
        "SELECT m.id, m.extension, m.media_type, m.codec, m.duration, t.value
         FROM media m
         LEFT JOIN tags t ON m.id = t.media_id AND t.key = 'description'
         WHERE {where_clause}
         ORDER BY m.imported_at ASC LIMIT 1 OFFSET ?1"
    );

    let result = conn.query_row(&list_sql, params![actual_index], |row| {
        let description: Option<String> = row.get(5)?;
        Ok(MediaInfo {
            id: row.get(0)?,
            extension: row.get(1)?,
            media_type: row.get(2)?,
            codec: row.get(3)?,
            has_description: description.is_some(),
            description,
            duration: row.get(4)?,
        })
    })?;

    Ok(Some(DescriptionPageData {
        description: result.description.clone(),
        media: result,
        current_index: actual_index,
        total_count: filtered_count,
    }))
}

pub fn get_media_index(
    conn: &Connection,
    media_id: &str,
    filter_missing_desc: bool,
    filter_missing_tags: bool,
) -> rusqlite::Result<Option<u32>> {
    let mut conditions = vec!["m.processed = 1".to_string()];
    if filter_missing_desc {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key = 'description')"
                .to_string(),
        );
    }
    if filter_missing_tags {
        conditions.push(
            "NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key NOT IN ('description', 'media_type'))"
                .to_string(),
        );
    }
    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT idx FROM (
            SELECT m.id, ROW_NUMBER() OVER (ORDER BY m.imported_at ASC) - 1 AS idx
            FROM media m WHERE {where_clause}
        ) WHERE id = ?1"
    );
    conn.query_row(&sql, params![media_id], |row| row.get(0))
        .optional()
}

// --- Embedding queries ---

pub fn insert_embedding(
    conn: &Connection,
    media_id: &str,
    embedding_type: &str,
    vector: &[u8],
    model_name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO embeddings (media_id, embedding_type, vector, model_name) VALUES (?1, ?2, ?3, ?4)",
        params![media_id, embedding_type, vector, model_name],
    )?;
    Ok(())
}

pub fn get_all_embeddings(conn: &Connection) -> rusqlite::Result<Vec<(String, String, Vec<u8>)>> {
    let mut stmt = conn.prepare(
        "SELECT e.media_id, e.embedding_type, e.vector FROM embeddings e
         JOIN media m ON e.media_id = m.id WHERE m.processed = 1"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    rows.collect()
}

/// Returns media IDs that have tags but need text embedding recomputed.
pub fn get_media_ids_needing_text_reembedding(
    conn: &Connection,
    model_name: &str,
) -> rusqlite::Result<Vec<(String, String)>> {
    // Find media that have at least one tag but missing/stale text embedding
    let mut stmt = conn.prepare(
        "SELECT DISTINCT m.id, 'has_tags' FROM media m
         JOIN tags t ON m.id = t.media_id
         LEFT JOIN embeddings e ON m.id = e.media_id AND e.embedding_type = 'text'
         WHERE e.media_id IS NULL OR e.model_name != ?1",
    )?;
    let rows = stmt.query_map(params![model_name], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect()
}

// --- Tag queries ---

pub fn get_tags(conn: &Connection, media_id: &str) -> rusqlite::Result<Vec<TagInfo>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM tags WHERE media_id = ?1 ORDER BY key, value",
    )?;
    let rows = stmt.query_map(params![media_id], |row| {
        Ok(TagInfo {
            key: row.get(0)?,
            value: row.get(1)?,
        })
    })?;
    rows.collect()
}

/// Replace all tags for a media item. Ensures tag keys exist in tag_keys table.
pub fn set_tags(conn: &Connection, media_id: &str, tags: &[TagInfo]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM tags WHERE media_id = ?1", params![media_id])?;
    for tag in tags {
        // Ensure key exists
        conn.execute(
            "INSERT OR IGNORE INTO tag_keys (key) VALUES (?1)",
            params![tag.key],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO tags (media_id, key, value) VALUES (?1, ?2, ?3)",
            params![media_id, tag.key, tag.value],
        )?;
    }
    Ok(())
}

pub fn get_all_tag_keys(conn: &Connection) -> rusqlite::Result<Vec<TagKeyInfo>> {
    let mut stmt = conn.prepare(
        "SELECT tk.key, COUNT(DISTINCT t.media_id) as cnt
         FROM tag_keys tk
         LEFT JOIN tags t ON tk.key = t.key
         GROUP BY tk.key
         ORDER BY tk.key",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TagKeyInfo {
            key: row.get(0)?,
            usage_count: row.get(1)?,
        })
    })?;
    rows.collect()
}

pub fn get_tag_values(conn: &Connection, key: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT value FROM tags WHERE key = ?1 ORDER BY value",
    )?;
    let rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
    rows.collect()
}

pub fn create_tag_key(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO tag_keys (key) VALUES (?1)", params![key])?;
    Ok(())
}

pub fn rename_tag_key(conn: &Connection, old_key: &str, new_key: &str) -> rusqlite::Result<()> {
    // Insert new key first, then update references, then remove old
    conn.execute("INSERT OR IGNORE INTO tag_keys (key) VALUES (?1)", params![new_key])?;
    conn.execute(
        "UPDATE tags SET key = ?1 WHERE key = ?2",
        params![new_key, old_key],
    )?;
    conn.execute("DELETE FROM tag_keys WHERE key = ?1", params![old_key])?;
    Ok(())
}

pub fn delete_tag_key(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM tags WHERE key = ?1", params![key])?;
    conn.execute("DELETE FROM tag_keys WHERE key = ?1", params![key])?;
    Ok(())
}

/// Assemble all tags for a media item into a single text for embedding.
/// Format: "key1: value1, value2. key2: value3."
pub fn assemble_tag_text(conn: &Connection, media_id: &str) -> rusqlite::Result<Option<String>> {
    let tags = get_tags(conn, media_id)?;
    if tags.is_empty() {
        return Ok(None);
    }

    // Group values by key, preserving order
    let mut key_order: Vec<String> = Vec::new();
    let mut grouped: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for tag in &tags {
        if !grouped.contains_key(&tag.key) {
            key_order.push(tag.key.clone());
        }
        grouped.entry(tag.key.clone()).or_default().push(tag.value.clone());
    }

    let parts: Vec<String> = key_order
        .iter()
        .map(|key| {
            let values = grouped.get(key).unwrap().join(", ");
            format!("{key}: {values}")
        })
        .collect();

    Ok(Some(parts.join(". ") + "."))
}

/// Returns (missing_desc, missing_tags, missing_both) counts for processed media.
pub fn get_missing_counts(conn: &Connection) -> rusqlite::Result<(u32, u32, u32)> {
    let missing_desc: u32 = conn.query_row(
        "SELECT COUNT(*) FROM media m WHERE m.processed = 1
         AND NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key = 'description')",
        [],
        |row| row.get(0),
    )?;
    let missing_tags: u32 = conn.query_row(
        "SELECT COUNT(*) FROM media m WHERE m.processed = 1
         AND NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key NOT IN ('description', 'media_type'))",
        [],
        |row| row.get(0),
    )?;
    let missing_both: u32 = conn.query_row(
        "SELECT COUNT(*) FROM media m WHERE m.processed = 1
         AND NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key = 'description')
         AND NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = m.id AND t.key NOT IN ('description', 'media_type'))",
        [],
        |row| row.get(0),
    )?;
    Ok((missing_desc, missing_tags, missing_both))
}

// --- Processing state ---

pub fn get_unprocessed_media(conn: &Connection) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, extension, media_type FROM media WHERE processed = 0 ORDER BY imported_at ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    rows.collect()
}

pub fn get_unprocessed_count(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("SELECT COUNT(*) FROM media WHERE processed = 0", [], |row| row.get(0))
}

pub fn get_processed_count(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("SELECT COUNT(*) FROM media WHERE processed = 1", [], |row| row.get(0))
}

pub fn mark_media_processed(conn: &Connection, media_id: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE media SET processed = 1 WHERE id = ?1", params![media_id])?;
    Ok(())
}

pub fn mark_media_unprocessed(conn: &Connection, media_id: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE media SET processed = 0 WHERE id = ?1", params![media_id])?;
    Ok(())
}
