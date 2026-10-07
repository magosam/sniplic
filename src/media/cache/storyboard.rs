use std::fs;
use std::path::Path;
use crate::core::paths::clean_path_for_ffmpeg;
use crate::error::AppResult;
use super::CacheManager;

impl CacheManager {
    /// Gets the storyboard path (6x4 sprite sheet of 24 frames) if it already exists on disk
    pub fn get_storyboard(&self, media_id: &str) -> Option<String> {
        let filename = format!("{}.jpg", media_id);
        let path = clean_path_for_ffmpeg(self.storyboard_dir.join(&filename));
        if path.exists() {
            Some(path.to_string_lossy().to_string())
        } else if self.project_id.is_some() {
            let global_path = clean_path_for_ffmpeg(crate::core::paths::get_storyboards_dir().join(&filename));
            if global_path.exists() {
                let _ = fs::copy(&global_path, &path);
                if path.exists() {
                    return Some(path.to_string_lossy().to_string());
                }
            }
            None
        } else {
            None
        }
    }

    /// Generates a 24-frame sprite sheet (6x4 grid at 1280x720 each in 720p HD) evenly distributed
    pub fn generate_storyboard<P: AsRef<Path>>(
        &self,
        media_id: &str,
        file_path: P,
        duration_seconds: f64,
    ) -> AppResult<Option<String>> {
        let filename = format!("{}.jpg", media_id);
        let output_path = clean_path_for_ffmpeg(self.storyboard_dir.join(&filename));

        if output_path.exists() {
            return Ok(Some(output_path.to_string_lossy().to_string()));
        }

        if self.project_id.is_some() {
            let global_path = clean_path_for_ffmpeg(crate::core::paths::get_storyboards_dir().join(&filename));
            if global_path.exists() {
                let _ = fs::copy(&global_path, &output_path);
                if output_path.exists() {
                    return Ok(Some(output_path.to_string_lossy().to_string()));
                }
            }
        }

        let clean_src = clean_path_for_ffmpeg(file_path.as_ref());
        let path_str = clean_src.to_string_lossy();
        let safe_dur = if duration_seconds > 0.05 { duration_seconds } else { 1.0 };
        let fps_val = 24.0 / safe_dur;

        let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        cmd.arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y");
        cmd.args([
            "-i",
            &path_str,
            "-an",
            "-vf",
            &format!(
                "fps={:.6},scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,tile=6x4",
                fps_val
            ),
            "-frames:v",
            "1",
        ])
        .arg(&output_path);

        if let Ok(status) = cmd.status() {
            if status.success() && output_path.exists() {
                return Ok(Some(output_path.to_string_lossy().to_string()));
            }
        }

        // Simplified fallback with select if the complex tile filter fails
        let mut fallback_cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        fallback_cmd
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .args([
                "-i",
                &path_str,
                "-vf",
                &format!("select='not(mod(n,{}))',scale=320:180,tile=6x4", (safe_dur * 24.0 / 24.0).max(1.0) as u32),
                "-frames:v",
                "1",
            ])
            .arg(&output_path);

        if let Ok(status) = fallback_cmd.status() {
            if status.success() && output_path.exists() {
                return Ok(Some(output_path.to_string_lossy().to_string()));
            }
        }

        Ok(None)
    }
}
