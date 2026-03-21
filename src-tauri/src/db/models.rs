use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub id: String,
    pub extension: String,
    pub media_type: String,
    pub codec: Option<String>,
    pub has_description: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub media: MediaInfo,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptionPageData {
    pub media: MediaInfo,
    pub description: Option<String>,
    pub current_index: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultInfo {
    pub path: String,
    pub media_count: u32,
}
