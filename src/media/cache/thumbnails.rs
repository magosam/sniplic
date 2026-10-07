use std::fs;
use std::path::Path;
use crate::core::paths::clean_path_for_ffmpeg;
use crate::error::AppResult;
use super::CacheManager;

impl CacheManager {
    pub fn generate_thumbnail<P: AsRef<Path>>(
        &self,
        media_id: &str,
        file_path: P,
    ) -> AppResult<Option<String>> {
        let thumb_filename = format!("{}.jpg", media_id);
        let thumb_path = clean_path_for_ffmpeg(self.cache_dir.join(&thumb_filename));

        if thumb_path.exists() {
            return Ok(Some(thumb_path.to_string_lossy().to_string()));
        }

        // Fallback: if in a project and the thumbnail already exists in the global cache, migrate to the project
        if self.project_id.is_some() {
            let global_thumb = clean_path_for_ffmpeg(crate::core::paths::get_thumbnails_dir().join(&thumb_filename));
            if global_thumb.exists() {
                let _ = fs::copy(&global_thumb, &thumb_path);
                if thumb_path.exists() {
                    return Ok(Some(thumb_path.to_string_lossy().to_string()));
                }
            }
        }

        let clean_src = clean_path_for_ffmpeg(file_path.as_ref());
        let path_str = clean_src.to_string_lossy();
        let is_image = matches!(
            clean_src.extension().and_then(|e| e.to_str()).unwrap_or(""),
            "png" | "jpg" | "jpeg" | "webp" | "bmp"
        );

        let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        cmd.arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y");

        if !is_image {
            cmd.args(["-ss", "00:00:01"]);
        }
        cmd.args([
            "-i",
            &path_str,
            "-frames:v",
            "1",
            "-vf",
            "scale='min(1280,iw)':-2",
            "-update",
            "1",
        ])
        .arg(&thumb_path);

        if let Ok(s) = cmd.status() {
            if s.success() && thumb_path.exists() {
                return Ok(Some(thumb_path.to_string_lossy().to_string()));
            }
        }

        Ok(None)
    }
}
