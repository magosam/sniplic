use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::info;

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static APP_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPathsInfo {
    pub app_data_dir: String,
    pub cache_dir: String,
    pub projects_dir: String,
    pub autosave_file: String,
    pub thumbnails_dir: String,
    pub storyboards_dir: String,
    pub waveforms_dir: String,
    pub scrub_proxies_dir: String,
    pub reversed_dir: String,
    pub extracted_audio_dir: String,
    pub fonts_dir: String,
}

/// Initializes the authoritative global paths from the Tauri application context.
pub fn init_app_paths(data_dir: PathBuf, cache_dir: PathBuf) {
    let _ = APP_DATA_DIR.set(data_dir.clone());
    let _ = APP_CACHE_DIR.set(cache_dir.clone());

    // Ensures that the basic directory structure exists immediately
    let _ = fs::create_dir_all(get_projects_dir());
    let _ = fs::create_dir_all(get_trash_dir());
    let _ = fs::create_dir_all(get_thumbnails_dir());
    let _ = fs::create_dir_all(get_storyboards_dir());
    let _ = fs::create_dir_all(get_waveforms_dir());
    let _ = fs::create_dir_all(get_scrub_proxies_dir());
    let _ = fs::create_dir_all(get_reversed_dir());
    let _ = fs::create_dir_all(get_extracted_audio_dir());
    let _ = fs::create_dir_all(get_app_fonts_dir());

    info!(
        "System directories initialized: data='{:?}', cache='{:?}'",
        get_app_data_dir(),
        get_app_cache_dir()
    );
}

pub fn get_app_data_dir() -> PathBuf {
    APP_DATA_DIR.get().cloned().unwrap_or_else(|| {
        if let Ok(app_data) = std::env::var("APPDATA") {
            PathBuf::from(app_data).join("com.sniplic.editor")
        } else if let Ok(local_data) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(local_data).join("com.sniplic.editor")
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .join(".editor_cache")
        }
    })
}

pub fn get_app_cache_dir() -> PathBuf {
    APP_CACHE_DIR.get().cloned().unwrap_or_else(|| {
        if let Ok(local_data) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(local_data).join("com.sniplic.editor").join("cache")
        } else {
            get_app_data_dir().join("cache")
        }
    })
}

pub fn get_projects_dir() -> PathBuf {
    get_app_data_dir().join("projects")
}

pub fn get_trash_dir() -> PathBuf {
    get_app_data_dir().join("trash")
}

pub fn get_autosave_file() -> PathBuf {
    get_app_data_dir().join("project_autosave.json")
}

pub fn get_thumbnails_dir() -> PathBuf {
    get_app_cache_dir().join("thumbnails")
}

pub fn get_storyboards_dir() -> PathBuf {
    get_app_cache_dir().join("storyboards")
}

pub fn get_waveforms_dir() -> PathBuf {
    get_app_cache_dir().join("waveforms")
}

pub fn get_scrub_proxies_dir() -> PathBuf {
    get_app_cache_dir().join("scrub_proxies")
}

pub fn get_reversed_dir() -> PathBuf {
    get_app_cache_dir().join("reversed")
}

pub fn get_extracted_audio_dir() -> PathBuf {
    get_app_cache_dir().join("extracted_audio")
}

pub fn get_app_fonts_dir() -> PathBuf {
    let p = get_app_data_dir().join("fonts");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_app_paths_info() -> AppPathsInfo {
    AppPathsInfo {
        app_data_dir: get_app_data_dir().to_string_lossy().to_string(),
        cache_dir: get_app_cache_dir().to_string_lossy().to_string(),
        projects_dir: get_projects_dir().to_string_lossy().to_string(),
        autosave_file: get_autosave_file().to_string_lossy().to_string(),
        thumbnails_dir: get_thumbnails_dir().to_string_lossy().to_string(),
        storyboards_dir: get_storyboards_dir().to_string_lossy().to_string(),
        waveforms_dir: get_waveforms_dir().to_string_lossy().to_string(),
        scrub_proxies_dir: get_scrub_proxies_dir().to_string_lossy().to_string(),
        reversed_dir: get_reversed_dir().to_string_lossy().to_string(),
        extracted_audio_dir: get_extracted_audio_dir().to_string_lossy().to_string(),
        fonts_dir: get_app_fonts_dir().to_string_lossy().to_string(),
    }
}

/// Removes the Windows extended prefix `\\?\` if present, for full compatibility with FFmpeg and other binaries.
pub fn clean_path_for_ffmpeg<P: AsRef<Path>>(path: P) -> PathBuf {
    let path_str = path.as_ref().to_string_lossy();
    if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.as_ref().to_path_buf()
    }
}

/// Returns the isolated base cache directory for a specific project.
/// Saves in `.editor_cache/projects/{project_id}/` if the workspace uses `.editor_cache`,
/// or in `get_app_cache_dir().join("projects").join(project_id)` as canonical default.
pub fn get_project_cache_dir(project_id: &str) -> PathBuf {
    let clean_id = project_id.trim();
    let cwd = std::env::current_dir().unwrap_or_default();
    
    let base = if cwd.join(".editor_cache").exists() {
        cwd.join(".editor_cache").join("projects").join(clean_id)
    } else if cwd.parent().map(|p| p.join(".editor_cache").exists()).unwrap_or(false) {
        cwd.parent().unwrap().join(".editor_cache").join("projects").join(clean_id)
    } else {
        get_app_cache_dir().join("projects").join(clean_id)
    };

    let _ = fs::create_dir_all(&base);
    clean_path_for_ffmpeg(&base)
}

pub fn get_project_mosaics_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("mosaics");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_thumbnails_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("thumbnails");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_waveforms_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("waveforms");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_scrub_proxies_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("scrub_proxies");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_storyboards_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("storyboards");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_reversed_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("reversed");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_extracted_audio_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("extracted_audio");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_media_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("media");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

pub fn get_project_bg_removed_dir(project_id: &str) -> PathBuf {
    let p = get_project_cache_dir(project_id).join("bg_removed");
    let _ = fs::create_dir_all(&p);
    clean_path_for_ffmpeg(&p)
}

/// Completely removes all cache directories and files associated with a project,
/// eliminating 100% of any leftovers (thumbnails, scrub proxies, waveforms, storyboards, reversed, mosaics, extracted audio, and media).
pub fn purge_all_project_cache(project_id: &str) {
    let clean_id = project_id.trim();
    if clean_id.is_empty() {
        return;
    }

    // 1. Remove project cache directory in workspace
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut ws_candidates = vec![
        cwd.join(".editor_cache").join("projects").join(clean_id),
        cwd.join(".editor_cache").join("cache").join(clean_id),
        cwd.join(".editor_cache").join("projects_cache").join(clean_id),
    ];
    if let Some(parent) = cwd.parent() {
        ws_candidates.push(parent.join(".editor_cache").join("projects").join(clean_id));
        ws_candidates.push(parent.join(".editor_cache").join("cache").join(clean_id));
        ws_candidates.push(parent.join(".editor_cache").join("projects_cache").join(clean_id));
    }
    for ws_dir in ws_candidates {
        if ws_dir.exists() {
            let _ = fs::remove_dir_all(&ws_dir);
            tracing::info!("Project cache '{}' removed from workspace at {:?}", clean_id, ws_dir);
        }
    }

    // 2. Remove project cache directory in AppData
    let app_data_project_cache = get_app_cache_dir().join("projects").join(clean_id);
    if app_data_project_cache.exists() {
        let _ = fs::remove_dir_all(&app_data_project_cache);
        tracing::info!("Project cache '{}' removed from AppData at {:?}", clean_id, app_data_project_cache);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathsInfo {
    pub project_id: String,
    pub cache_dir: String,
    pub mosaics_dir: String,
    pub thumbnails_dir: String,
    pub waveforms_dir: String,
    pub scrub_proxies_dir: String,
    pub storyboards_dir: String,
    pub reversed_dir: String,
    pub extracted_audio_dir: String,
    pub media_dir: String,
}

pub fn get_project_paths_info(project_id: &str) -> ProjectPathsInfo {
    ProjectPathsInfo {
        project_id: project_id.to_string(),
        cache_dir: get_project_cache_dir(project_id).to_string_lossy().to_string(),
        mosaics_dir: get_project_mosaics_dir(project_id).to_string_lossy().to_string(),
        thumbnails_dir: get_project_thumbnails_dir(project_id).to_string_lossy().to_string(),
        waveforms_dir: get_project_waveforms_dir(project_id).to_string_lossy().to_string(),
        scrub_proxies_dir: get_project_scrub_proxies_dir(project_id).to_string_lossy().to_string(),
        storyboards_dir: get_project_storyboards_dir(project_id).to_string_lossy().to_string(),
        reversed_dir: get_project_reversed_dir(project_id).to_string_lossy().to_string(),
        extracted_audio_dir: get_project_extracted_audio_dir(project_id).to_string_lossy().to_string(),
        media_dir: get_project_media_dir(project_id).to_string_lossy().to_string(),
    }
}

/// Transparently migrates files from legacy cache (.editor_cache) to new canonical directories
/// if there are projects or thumbnails not yet present in the AppData directory.
/// Executed only once via sentinel file to avoid ever reviving deleted files.
pub fn migrate_legacy_cache_if_needed() {
    let sentinel = get_app_data_dir().join(".legacy_migrated");
    if sentinel.exists() {
        return;
    }

    let target_projects = get_projects_dir();
    let target_autosave = get_autosave_file();
    let target_thumbnails = get_thumbnails_dir();

    let legacy_candidates = [
        PathBuf::from(".editor_cache"),
        PathBuf::from("src-tauri/.editor_cache"),
    ];

    for legacy in &legacy_candidates {
        if !legacy.exists() {
            continue;
        }

        // 1. Project migration (.json)
        let legacy_projects = legacy.join("projects");
        if legacy_projects.exists() && legacy_projects.is_dir() {
            migrate_directory_recursive(&legacy_projects, &target_projects);
        }

        // 2. Autosave migration
        let legacy_autosave = legacy.join("project_autosave.json");
        if legacy_autosave.exists() && !target_autosave.exists() {
            if let Ok(_) = fs::copy(&legacy_autosave, &target_autosave) {
                info!("Legacy autosave successfully migrated from {:?} to {:?}", legacy_autosave, target_autosave);
            }
        }

        // 3. Existing thumbnails migration
        let legacy_thumbs = legacy.join("thumbnails");
        if legacy_thumbs.exists() && legacy_thumbs.is_dir() {
            if let Ok(entries) = fs::read_dir(&legacy_thumbs) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(file_name) = p.file_name() {
                            let dest = target_thumbnails.join(file_name);
                            if !dest.exists() {
                                let _ = fs::copy(&p, &dest);
                            }
                        }
                    }
                }
            }
        }
    }

    // Marks migration as permanently completed
    let _ = fs::write(&sentinel, "migrated");
}

fn migrate_directory_recursive(src: &Path, dst: &Path) {
    if !src.exists() || !src.is_dir() {
        return;
    }
    let _ = fs::create_dir_all(dst);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            if let Some(file_name) = src_path.file_name() {
                let dst_path = dst.join(file_name);
                if src_path.is_dir() {
                    migrate_directory_recursive(&src_path, &dst_path);
                } else if src_path.is_file() && src_path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if !dst_path.exists() {
                        if let Ok(_) = fs::copy(&src_path, &dst_path) {
                            info!("Project successfully migrated from {:?} to {:?}", src_path, dst_path);
                        }
                    }
                }
            }
        }
    }
}

pub fn get_app_paths() -> AppPathsInfo {
    get_app_paths_info()
}

pub fn get_project_paths(project_id: String) -> ProjectPathsInfo {
    get_project_paths_info(&project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_paths_resolution() {
        let app_data = get_app_data_dir();
        let app_cache = get_app_cache_dir();
        let projects = get_projects_dir();
        let autosave = get_autosave_file();
        let thumbs = get_thumbnails_dir();

        assert_eq!(projects, app_data.join("projects"));
        assert_eq!(autosave, app_data.join("project_autosave.json"));
        assert_eq!(thumbs, app_cache.join("thumbnails"));
        assert_eq!(get_app_fonts_dir(), app_data.join("fonts"));
    }

    #[test]
    fn test_get_app_paths_info_serializes() {
        let info = get_app_paths_info();
        assert!(!info.app_data_dir.is_empty());
        assert!(!info.cache_dir.is_empty());
        assert!(!info.thumbnails_dir.is_empty());
        assert!(!info.fonts_dir.is_empty());

        let json = serde_json::to_string(&info).expect("Should serialize AppPathsInfo");
        assert!(json.contains("thumbnailsDir"));
        assert!(json.contains("appDataDir"));
        assert!(json.contains("fontsDir"));
    }

    #[test]
    fn test_migrate_directory_recursive() {
        let temp_dir = std::env::temp_dir().join(format!("sniplic_test_mig_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let src_dir = temp_dir.join("src");
        let dst_dir = temp_dir.join("dst");

        fs::create_dir_all(&src_dir).unwrap();
        let dummy_json = src_dir.join("test_project.json");
        fs::write(&dummy_json, "{}").unwrap();

        migrate_directory_recursive(&src_dir, &dst_dir);

        let copied_json = dst_dir.join("test_project.json");
        assert!(copied_json.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_cache_isolation_and_purge() {
        let proj_id = format!("test_proj_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let info = get_project_paths_info(&proj_id);
        assert_eq!(info.project_id, proj_id);
        assert!(info.cache_dir.contains(&proj_id));
        assert!(info.mosaics_dir.contains("mosaics"));
        assert!(info.thumbnails_dir.contains("thumbnails"));

        let serialized = serde_json::to_string(&info).expect("Should serialize ProjectPathsInfo");
        assert!(serialized.contains("mosaicsDir"));
        assert!(serialized.contains("scrubProxiesDir"));

        // Create project cache files
        let cache_dir = get_project_cache_dir(&proj_id);
        let mosaic_dir = get_project_mosaics_dir(&proj_id);
        fs::create_dir_all(&mosaic_dir).unwrap();
        let dummy_mosaic = mosaic_dir.join("mosaic_001.jpg");
        fs::write(&dummy_mosaic, b"fake-mosaic").unwrap();
        assert!(dummy_mosaic.exists());

        // Execute total purge of project cache
        purge_all_project_cache(&proj_id);
        assert!(!cache_dir.exists(), "Project cache should be completely removed");
        assert!(!dummy_mosaic.exists(), "Files inside cache should have been deleted");
    }
}
