//! Database operations for chat history

use crate::models::ChatMessage;
use crate::paths::{get_db_path, get_db_path_for_character};
use rusqlite::{params, Connection};

/// Initializes the SQLite database, creating tables if needed
pub fn init_database() -> Result<Connection, String> {
    init_database_for_character("character_1")
}

/// Initializes the SQLite database for a specific character, creating tables if needed
pub fn init_database_for_character(character_id: &str) -> Result<Connection, String> {
    let db_path = if character_id == "character_1" {
        // Backward compatibility path for legacy installs
        get_db_path_for_character(character_id).unwrap_or(get_db_path()?)
    } else {
        get_db_path_for_character(character_id)?
    };

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create database directory: {}", e))?;
    }

    let conn = Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS chat_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            context_level INTEGER DEFAULT 0
        )",
        [],
    )
    .map_err(|e| format!("Failed to create table: {}", e))?;

    // Migration: Add context_level column if it doesn't exist (for existing databases)
    let _ = conn.execute(
        "ALTER TABLE chat_history ADD COLUMN context_level INTEGER DEFAULT 0",
        [],
    ); // Ignore error if column already exists

    Ok(conn)
}

/// Stores a chat message in the database
pub fn store_chat_message(
    timestamp: &str,
    role: &str,
    content: &str,
    context_level: u8,
) -> Result<(), String> {
    store_chat_message_for_character("character_1", timestamp, role, content, context_level)
}

/// Stores a chat message for a specific character
pub fn store_chat_message_for_character(
    character_id: &str,
    timestamp: &str,
    role: &str,
    content: &str,
    context_level: u8,
) -> Result<(), String> {
    let conn = init_database_for_character(character_id)?;
    conn.execute(
        "INSERT INTO chat_history (timestamp, role, content, context_level) VALUES (?1, ?2, ?3, ?4)",
        params![timestamp, role, content, context_level],
    ).map_err(|e| format!("Failed to store message: {}", e))?;
    Ok(())
}

/// Retrieves chat history from the database
pub fn get_chat_history_internal(limit: i64) -> Result<Vec<ChatMessage>, String> {
    get_chat_history_internal_for_character("character_1", limit)
}

/// Retrieves chat history for a specific character
pub fn get_chat_history_internal_for_character(
    character_id: &str,
    limit: i64,
) -> Result<Vec<ChatMessage>, String> {
    let conn = init_database_for_character(character_id)?;
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, role, content, COALESCE(context_level, 0) FROM chat_history ORDER BY id DESC LIMIT ?1"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let messages = stmt
        .query_map(params![limit], |row| {
            Ok(ChatMessage {
                id: Some(row.get(0)?),
                timestamp: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                context_level: row.get::<_, i64>(4)? as u8,
            })
        })
        .map_err(|e| format!("Failed to query: {}", e))?;

    let mut result: Vec<ChatMessage> = messages.filter_map(|m| m.ok()).collect();

    // Reverse to get chronological order
    result.reverse();
    Ok(result)
}

/// Clears all chat history from the database
pub fn clear_chat_history_internal() -> Result<(), String> {
    clear_chat_history_internal_for_character("character_1")
}

/// Clears all chat history for a specific character
pub fn clear_chat_history_internal_for_character(character_id: &str) -> Result<(), String> {
    let conn = init_database_for_character(character_id)?;
    conn.execute("DELETE FROM chat_history", [])
        .map_err(|e| format!("Failed to clear history: {}", e))?;
    Ok(())
}
