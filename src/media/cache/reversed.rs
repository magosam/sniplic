use std::fs;
use std::path::Path;
use crate::core::paths::clean_path_for_ffmpeg;
use crate::error::AppResult;
use super::CacheManager;

impl CacheManager {
    /// Generates (or returns from cache) a copy with reversed audio/video
    /// using ultrafast FFmpeg for smooth preview and playback at 60 FPS without freezing the playhead.
    pub fn get_or_generate_reversed_media<P: AsRef<Path>>(
        &self,
        media_id: &str,
        file_path: P,
        is_audio: bool,
    ) -> AppResult<Option<String>> {
        let ext = if is_audio { "m4a" } else { "mp4" };
        let reversed_filename = format!("{}.{}", media_id, ext);
        let reversed_path = clean_path_for_ffmpeg(self.reversed_dir.join(&reversed_filename));

        if reversed_path.exists() {
            return Ok(Some(reversed_path.to_string_lossy().to_string()));
        }

        if self.project_id.is_some() {
            let global_path = clean_path_for_ffmpeg(crate::core::paths::get_reversed_dir().join(&reversed_filename));
            if global_path.exists() {
                let _ = fs::copy(&global_path, &reversed_path);
                if reversed_path.exists() {
                    return Ok(Some(reversed_path.to_string_lossy().to_string()));
                }
            }
        }

        let clean_src = clean_path_for_ffmpeg(file_path.as_ref());
        let path_str = clean_src.to_string_lossy();

        let mut cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
        cmd.arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y");

        cmd.args(["-i", &path_str]);

        if is_audio {
            cmd.args(["-vn", "-af", "areverse", "-c:a", "aac", "-b:a", "192k"]);
        } else {
            cmd.args([
                "-vf", "reverse",
                "-af", "areverse",
                "-c:v", "libx264",
                "-preset", "ultrafast",
                "-tune", "fastdecode",
                "-c:a", "aac",
                "-b:a", "192k",
                "-pix_fmt", "yuv420p",
                "-movflags", "+faststart",
            ]);
        }

        cmd.arg(&reversed_path);

        let output = cmd.output();

        match output {
            Ok(out) if out.status.success() && reversed_path.exists() => {
                Ok(Some(reversed_path.to_string_lossy().to_string()))
            }
            _ => Ok(None),
        }
    }
}
