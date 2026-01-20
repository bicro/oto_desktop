//! Data models and structures used throughout the application

use serde::{Deserialize, Serialize};

/// Represents a single chat message stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Option<i64>,
    pub timestamp: String,
    pub role: String,
    pub content: String,
    pub context_level: u8,
}

/// Response from the chat API including optional character comments
#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub main_response: String,
    pub character_comments: Option<Vec<String>>,
}

/// Version information for a saved texture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureVersion {
    pub id: String,
    pub created_at: String,
    pub prompt: Option<String>,
}
