use rusqlite::{params, Connection};

use super::models::{DescriptionPageData, MediaInfo, VaultInfo};

pub fn get_vault_info(conn: &Connection, path: &str) -> rusqlite::Result<VaultInfo> {
    let count: u32 =
        conn.query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))?;
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
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO media (id, extension, media_type, codec, file_size, checksum) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, extension, media_type, codec, file_size.map(|s| s as i64), checksum],
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
        "SELECT id, extension, media_type, codec, description
         FROM media ORDER BY imported_at DESC LIMIT ?1 OFFSET ?2",
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
        })
    })?;
    rows.collect()
}

pub fn get_media_by_id(conn: &Connection, media_id: &str) -> rusqlite::Result<Option<MediaInfo>> {
    conn.query_row(
        "SELECT id, extension, media_type, codec, description
         FROM media WHERE id = ?1",
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
            })
        },
    )
    .optional()
}

pub fn delete_media(conn: &Connection, media_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM media WHERE id = ?1", params![media_id])?;
    Ok(())
}

pub fn get_description(conn: &Connection, media_id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT description FROM media WHERE id = ?1",
        params![media_id],
        |row| row.get(0),
    )
    .optional()
    .map(|o| o.flatten())
}

pub fn set_description(
    conn: &Connection,
    media_id: &str,
    description: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media SET description = ?1 WHERE id = ?2",
        params![description, media_id],
    )?;
    Ok(())
}

pub fn get_media_for_description(
    conn: &Connection,
    index: u32,
    filter_missing: bool,
) -> rusqlite::Result<Option<DescriptionPageData>> {
    let (count_sql, list_sql) = if filter_missing {
        (
            "SELECT COUNT(*) FROM media WHERE description IS NULL",
            "SELECT id, extension, media_type, codec, description
             FROM media WHERE description IS NULL ORDER BY imported_at ASC LIMIT 1 OFFSET ?1",
        )
    } else {
        (
            "SELECT COUNT(*) FROM media",
            "SELECT id, extension, media_type, codec, description
             FROM media ORDER BY imported_at ASC LIMIT 1 OFFSET ?1",
        )
    };

    let total_count: u32 = conn.query_row(count_sql, [], |row| row.get(0))?;
    if total_count == 0 {
        return Ok(None);
    }

    let actual_index = index.min(total_count - 1);

    let result = conn.query_row(list_sql, params![actual_index], |row| {
        let description: Option<String> = row.get(4)?;
        Ok(DescriptionPageData {
            media: MediaInfo {
                id: row.get(0)?,
                extension: row.get(1)?,
                media_type: row.get(2)?,
                codec: row.get(3)?,
                has_description: description.is_some(),
                description: description.clone(),
            },
            description,
            current_index: actual_index,
            total_count,
        })
    })?;

    Ok(Some(result))
}

pub fn insert_embedding(
    conn: &Connection,
    media_id: &str,
    vector: &[u8],
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO embeddings (media_id, vector) VALUES (?1, ?2)",
        params![media_id, vector],
    )?;
    Ok(())
}

pub fn get_all_embeddings(conn: &Connection) -> rusqlite::Result<Vec<(String, Vec<u8>)>> {
    let mut stmt = conn.prepare("SELECT media_id, vector FROM embeddings")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    rows.collect()
}

pub fn get_media_index(
    conn: &Connection,
    media_id: &str,
    filter_missing: bool,
) -> rusqlite::Result<Option<u32>> {
    let sql = if filter_missing {
        "SELECT idx FROM (
            SELECT id, ROW_NUMBER() OVER (ORDER BY imported_at ASC) - 1 AS idx
            FROM media WHERE description IS NULL
        ) WHERE id = ?1"
    } else {
        "SELECT idx FROM (
            SELECT id, ROW_NUMBER() OVER (ORDER BY imported_at ASC) - 1 AS idx
            FROM media
        ) WHERE id = ?1"
    };
    conn.query_row(sql, params![media_id], |row| row.get(0))
        .optional()
}

use rusqlite::OptionalExtension;
