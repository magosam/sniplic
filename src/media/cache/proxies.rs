use std::fs;
use std::path::Path;
use crate::core::paths::clean_path_for_ffmpeg;
use crate::error::AppResult;
use super::CacheManager;

impl CacheManager {
    /// Generates (or returns from cache) a "scrub proxy": an all-intra H.264 MP4
    /// (GOP=1, no B-frames), at reduced resolution and without audio.
    pub fn get_or_generate_scrub_proxy<P: AsRef<Path>>(
        &self,
        media_id: &str,
        file_path: P,
    ) -> AppResult<Option<String>> {
        let proxy_filename = format!("{}.mp4", media_id);
        let proxy_path = clean_path_for_ffmpeg(self.scrub_proxy_dir.join(&proxy_filename));

        if proxy_path.exists() {
            return Ok(Some(proxy_path.to_string_lossy().to_string()));
        }

        if self.project_id.is_some() {
            let global_path = clean_path_for_ffmpeg(crate::core::paths::get_scrub_proxies_dir().join(&proxy_filename));
            if global_path.exists() {
                let _ = fs::copy(&global_path, &proxy_path);
                if proxy_path.exists() {
                    return Ok(Some(proxy_path.to_string_lossy().to_string()));
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

        cmd.args(["-i", &path_str])
            .args(["-an"]) // no audio: preview plays the audio of the original file in parallel
            .args(["-vf", "scale='min(1280,iw)':-2"]) // 720p HD resolution, preserves aspect ratio, even height
            .args(["-c:v", "libx264"])
            .args(["-preset", "ultrafast"])
            .args(["-tune", "fastdecode"])
            .args(["-g", "1"]) // GOP=1: every frame is an independent keyframe
            .args(["-keyint_min", "1"])
            .args(["-sc_threshold", "0"])
            .args(["-bf", "0"]) // no B-frames: eliminates any reordering dependency
            .args(["-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(&proxy_path);

        let output = cmd.output();

        match output {
            Ok(out) if out.status.success() && proxy_path.exists() => {
                Ok(Some(proxy_path.to_string_lossy().to_string()))
            }
            _ => {
                // Simplified fallback with mpeg4 if the libx264 encoder fails in the local environment
                let mut fallback_cmd = crate::ffmpeg::binary::get_ffmpeg_cmd();
                fallback_cmd
                    .arg("-hide_banner")
                    .arg("-loglevel")
                    .arg("error")
                    .arg("-y")
                    .args(["-i", &path_str])
                    .args(["-an"])
                    .args(["-vf", "scale='min(640,iw)':-2"])
                    .args(["-c:v", "mpeg4"])
                    .args(["-q:v", "5"])
                    .args(["-g", "1"])
                    .arg(&proxy_path);

                let fb_out = fallback_cmd.output();
                if let Ok(out) = fb_out {
                    if out.status.success() && proxy_path.exists() {
                        return Ok(Some(proxy_path.to_string_lossy().to_string()));
                    }
                }

                Ok(None)
            }
        }
    }
}
