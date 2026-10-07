use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MediaType {
    Video,
    Audio,
    Image,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MediaItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "file_path")]
    pub file_path: PathBuf,
    #[serde(rename = "media_type")]
    pub media_type: MediaType,
    #[serde(rename = "duration_frames")]
    pub duration_frames: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(default, rename = "sample_rate", skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
    #[serde(default, rename = "added_at")]
    pub added_at: u64,
    #[serde(default, rename = "folder_id", alias = "folderId", skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

impl<'de> Deserialize<'de> for MediaItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let file_path = v.get("file_path").or_else(|| v.get("filePath"))
            .and_then(|x| x.as_str())
            .map(PathBuf::from)
            .unwrap_or_default();
        let media_type = v.get("media_type").or_else(|| v.get("mediaType"))
            .and_then(|x| serde_json::from_value(x.clone()).ok())
            .unwrap_or(MediaType::Video);

        let parse_u64 = |val: Option<&serde_json::Value>| -> u64 {
            val.and_then(|x| {
                x.as_u64()
                    .or_else(|| x.as_f64().map(|f| f.max(0.0).round() as u64))
                    .or_else(|| x.as_i64().map(|i| i.max(0) as u64))
            })
            .unwrap_or(0)
        };

        let duration_frames = parse_u64(v.get("duration_frames").or_else(|| v.get("durationFrames")));
        let width = v.get("width").and_then(|x| x.as_u64().map(|u| u as u32));
        let height = v.get("height").and_then(|x| x.as_u64().map(|u| u as u32));
        let fps = v.get("fps").and_then(|x| x.as_f64());
        let sample_rate = v.get("sample_rate").or_else(|| v.get("sampleRate"))
            .and_then(|x| x.as_u64().map(|u| u as u32));
        let channels = v.get("channels").and_then(|x| x.as_u64().map(|u| u as u16));
        let added_at = parse_u64(v.get("added_at").or_else(|| v.get("addedAt")));
        let folder_id = v.get("folder_id").or_else(|| v.get("folderId"))
            .and_then(|x| x.as_str().map(|s| s.to_string()));

        Ok(MediaItem {
            id,
            name,
            file_path,
            media_type,
            duration_frames,
            width,
            height,
            fps,
            sample_rate,
            channels,
            added_at,
            folder_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MediaFolder {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "created_at", alias = "createdAt")]
    pub created_at: u64,
    #[serde(default, rename = "parent_id", alias = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

impl<'de> Deserialize<'de> for MediaFolder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let parse_u64 = |val: Option<&serde_json::Value>| -> u64 {
            val.and_then(|x| {
                x.as_u64()
                    .or_else(|| x.as_f64().map(|f| f.max(0.0).round() as u64))
                    .or_else(|| x.as_i64().map(|i| i.max(0) as u64))
            })
            .unwrap_or(0)
        };
        let created_at = parse_u64(v.get("created_at").or_else(|| v.get("createdAt")));
        let parent_id = v.get("parent_id").or_else(|| v.get("parentId"))
            .and_then(|x| x.as_str().map(|s| s.to_string()));

        Ok(MediaFolder {
            id,
            name,
            created_at,
            parent_id,
        })
    }
}

