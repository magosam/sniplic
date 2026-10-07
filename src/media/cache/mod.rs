use std::fs;
use std::path::PathBuf;

pub mod proxies;
pub mod reversed;
pub mod storyboard;
pub mod thumbnails;
pub mod waveforms;

pub struct CacheManager {
    pub(crate) project_id: Option<String>,
    pub(crate) cache_dir: PathBuf,
    pub(crate) waveform_dir: PathBuf,
    pub(crate) scrub_proxy_dir: PathBuf,
    pub(crate) reversed_dir: PathBuf,
    pub(crate) storyboard_dir: PathBuf,
    #[allow(dead_code)]
    pub(crate) mosaics_dir: PathBuf,
    pub(crate) extracted_audio_dir: PathBuf,
    #[allow(dead_code)]
    pub(crate) media_dir: PathBuf,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheManager {
    /// Initializes the CacheManager with isolated scope for a specific project.
    /// All artifacts (mosaics, thumbnails, scrub proxies, waveforms, reversed,
    /// storyboards, extracted_audio, and media) are organized within the project directory.
    pub fn for_project(project_id: &str) -> Self {
        let clean_id = project_id.trim();
        if clean_id.is_empty() {
            return Self::new();
        }

        let cache_dir = crate::core::paths::get_project_thumbnails_dir(clean_id);
        let waveform_dir = crate::core::paths::get_project_waveforms_dir(clean_id);
        let scrub_proxy_dir = crate::core::paths::get_project_scrub_proxies_dir(clean_id);
        let reversed_dir = crate::core::paths::get_project_reversed_dir(clean_id);
        let storyboard_dir = crate::core::paths::get_project_storyboards_dir(clean_id);
        let mosaics_dir = crate::core::paths::get_project_mosaics_dir(clean_id);
        let extracted_audio_dir = crate::core::paths::get_project_extracted_audio_dir(clean_id);
        let media_dir = crate::core::paths::get_project_media_dir(clean_id);

        let _ = fs::create_dir_all(&cache_dir);
        let _ = fs::create_dir_all(&waveform_dir);
        let _ = fs::create_dir_all(&scrub_proxy_dir);
        let _ = fs::create_dir_all(&reversed_dir);
        let _ = fs::create_dir_all(&storyboard_dir);
        let _ = fs::create_dir_all(&mosaics_dir);
        let _ = fs::create_dir_all(&extracted_audio_dir);
        let _ = fs::create_dir_all(&media_dir);

        Self {
            project_id: Some(clean_id.to_string()),
            cache_dir,
            waveform_dir,
            scrub_proxy_dir,
            reversed_dir,
            storyboard_dir,
            mosaics_dir,
            extracted_audio_dir,
            media_dir,
        }
    }

    /// Initializes the default CacheManager (used as fallback when no project is specified)
    pub fn new() -> Self {
        let cache_dir = crate::core::paths::get_thumbnails_dir();
        let waveform_dir = crate::core::paths::get_waveforms_dir();
        let scrub_proxy_dir = crate::core::paths::get_scrub_proxies_dir();
        let reversed_dir = crate::core::paths::get_reversed_dir();
        let storyboard_dir = crate::core::paths::get_storyboards_dir();
        let mosaics_dir = crate::core::paths::get_app_cache_dir().join("mosaics");
        let extracted_audio_dir = crate::core::paths::get_extracted_audio_dir();
        let media_dir = crate::core::paths::get_app_data_dir().join("media");

        let _ = fs::create_dir_all(&cache_dir);
        let _ = fs::create_dir_all(&waveform_dir);
        let _ = fs::create_dir_all(&scrub_proxy_dir);
        let _ = fs::create_dir_all(&reversed_dir);
        let _ = fs::create_dir_all(&storyboard_dir);
        let _ = fs::create_dir_all(&mosaics_dir);
        let _ = fs::create_dir_all(&extracted_audio_dir);
        let _ = fs::create_dir_all(&media_dir);

        Self {
            project_id: None,
            cache_dir,
            waveform_dir,
            scrub_proxy_dir,
            reversed_dir,
            storyboard_dir,
            mosaics_dir,
            extracted_audio_dir,
            media_dir,
        }
    }

    /// Removes all cache residues of a specific media item within this project
    pub fn remove_media_cache(&self, media_id: &str) {
        let thumb = self.cache_dir.join(format!("{}.jpg", media_id));
        if thumb.exists() { let _ = fs::remove_file(thumb); }

        let wave = self.waveform_dir.join(format!("{}.json", media_id));
        if wave.exists() { let _ = fs::remove_file(wave); }

        let story = self.storyboard_dir.join(format!("{}.jpg", media_id));
        if story.exists() { let _ = fs::remove_file(story); }

        let proxy = self.scrub_proxy_dir.join(format!("{}.mp4", media_id));
        if proxy.exists() { let _ = fs::remove_file(proxy); }

        let rev_mp4 = self.reversed_dir.join(format!("{}.mp4", media_id));
        if rev_mp4.exists() { let _ = fs::remove_file(rev_mp4); }

        let rev_m4a = self.reversed_dir.join(format!("{}.m4a", media_id));
        if rev_m4a.exists() { let _ = fs::remove_file(rev_m4a); }

        let ext_wav = self.extracted_audio_dir.join(format!("{}.wav", media_id));
        if ext_wav.exists() { let _ = fs::remove_file(ext_wav); }
    }

    pub fn remove_scrub_proxy(&self, media_id: &str) {
        let proxy = self.scrub_proxy_dir.join(format!("{}.mp4", media_id));
        if proxy.exists() { let _ = fs::remove_file(proxy); }
    }

    pub fn remove_reversed_media(&self, media_id: &str) {
        let rev_mp4 = self.reversed_dir.join(format!("{}.mp4", media_id));
        if rev_mp4.exists() { let _ = fs::remove_file(rev_mp4); }
        let rev_m4a = self.reversed_dir.join(format!("{}.m4a", media_id));
        if rev_m4a.exists() { let _ = fs::remove_file(rev_m4a); }
    }

    pub fn remove_storyboard(&self, media_id: &str) {
        let story = self.storyboard_dir.join(format!("{}.jpg", media_id));
        if story.exists() { let _ = fs::remove_file(story); }
    }
}
