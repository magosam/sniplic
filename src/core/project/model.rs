use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use crate::error::AppResult;

use super::media::{MediaFolder, MediaItem, MediaType};
use super::subtitles::ProjectSubtitles;
use super::timeline::{Track, TrackType};

fn default_revision() -> u64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default = "default_revision")]
    pub revision: u64,
    #[serde(default, rename = "created_at", alias = "createdAt")]
    pub created_at: u64,
    #[serde(default, rename = "updated_at", alias = "updatedAt")]
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub sample_rate: u32,
    pub audio_channels: u16,
    #[serde(default = "default_image_duration", rename = "default_image_duration", alias = "defaultImageDuration")]
    pub default_image_duration: f64,
}

fn default_image_duration() -> f64 {
    5.0
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30.0,
            sample_rate: 48000,
            audio_channels: 2,
            default_image_duration: 5.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineMarker {
    pub id: String,
    pub frame: u64,
    pub label: String,
    #[serde(default = "default_marker_color")]
    pub color: String,
}

fn default_marker_color() -> String {
    "blue".to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Project {
    pub metadata: ProjectMetadata,
    pub config: ProjectConfig,
    #[serde(rename = "media_pool", alias = "mediaPool")]
    pub media_pool: HashMap<String, MediaItem>,
    #[serde(default, rename = "media_folders", alias = "mediaFolders", skip_serializing_if = "Option::is_none")]
    pub media_folders: Option<Vec<MediaFolder>>,
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<ProjectSubtitles>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markers: Option<Vec<TimelineMarker>>,
}

impl<'de> Deserialize<'de> for Project {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let metadata = v.get("metadata")
            .and_then(|m| serde_json::from_value(m.clone()).ok())
            .unwrap_or_else(|| ProjectMetadata {
                id: String::new(),
                name: String::new(),
                version: "1.0.0".to_string(),
                revision: 1,
                created_at: 0,
                updated_at: 0,
            });
        let config = v.get("config")
            .and_then(|c| serde_json::from_value(c.clone()).ok())
            .unwrap_or_default();
        let media_pool = v.get("media_pool").or_else(|| v.get("mediaPool"))
            .and_then(|mp| serde_json::from_value(mp.clone()).ok())
            .unwrap_or_default();
        let media_folders = v.get("media_folders").or_else(|| v.get("mediaFolders"))
            .and_then(|mf| serde_json::from_value(mf.clone()).ok());
        let tracks = v.get("tracks")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();
        let subtitles = v.get("subtitles")
            .and_then(|s| serde_json::from_value(s.clone()).ok());
        let markers = v.get("markers")
            .and_then(|m| serde_json::from_value(m.clone()).ok());

        Ok(Project {
            metadata,
            config,
            media_pool,
            media_folders,
            tracks,
            subtitles,
            markers,
        })
    }
}


impl Project {
    pub fn bump_revision(&mut self) -> u64 {
        self.metadata.revision += 1;
        self.metadata.updated_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.metadata.revision
    }

    pub fn new(name: String) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            metadata: ProjectMetadata {
                id: format!("prj_{:x}", now),
                name,
                version: "1.0.0".to_string(),
                revision: 1,
                created_at: now,
                updated_at: now,
            },
            config: ProjectConfig::default(),
            media_pool: HashMap::new(),
            media_folders: Some(Vec::new()),
            tracks: vec![
                Track {
                    id: "v1".to_string(),
                    name: "V1".to_string(),
                    track_type: TrackType::Video,
                    locked: false,
                    muted: false,
                    hidden: false,
                    solo: false,
                    clips: Vec::new(),
                    height: None,
                },
                Track {
                    id: "a1".to_string(),
                    name: "A1".to_string(),
                    track_type: TrackType::Audio,
                    locked: false,
                    muted: false,
                    hidden: false,
                    solo: false,
                    clips: Vec::new(),
                    height: None,
                },
            ],
            subtitles: Some(ProjectSubtitles {
                items: Vec::new(),
                ..ProjectSubtitles::default()
            }),
            markers: None,
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> AppResult<()> {
        let json_str = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json_str)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> AppResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let project: Project = serde_json::from_str(&content)?;
        Ok(project)
    }
}

pub type SharedProjectState = Arc<RwLock<Option<Project>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMediaPreview {
    pub id: String,
    pub media_type: MediaType,
    pub file_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub updated_at: u64,
    pub clip_count: usize,
    pub track_count: usize,
    #[serde(default)]
    pub duration_seconds: u64,
    #[serde(default)]
    pub timeline_previews: Vec<ProjectMediaPreview>,
    #[serde(default, rename = "folderPath", alias = "folder_path", skip_serializing_if = "Option::is_none")]
    pub folder_path: Option<String>,
}
