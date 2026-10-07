use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

static CACHED_FFMPEG_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
static CACHED_FFPROBE_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Clears the path cache to force a new detection (e.g., after installation or automatic download)
pub fn reset_cached_paths() {
    if let Ok(mut lock) = CACHED_FFMPEG_PATH.write() {
        *lock = None;
    }
    if let Ok(mut lock) = CACHED_FFPROBE_PATH.write() {
        *lock = None;
    }
}

/// Checks if an executable path actually exists and is executable
pub fn is_valid_executable(path: &Path) -> bool {
    if !path.exists() || !path.is_file() {
        return false;
    }
    let mut cmd = Command::new(path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.arg("-version");
    cmd.output().map(|out| out.status.success()).unwrap_or(false)
}

/// Checks if FFmpeg is installed and functional on the operating system
pub fn is_ffmpeg_available() -> bool {
    let path = find_ffmpeg_path();
    if is_valid_executable(&path) {
        return true;
    }

    // If the path is only the relative name "ffmpeg", tests direct execution in PATH
    let mut cmd = Command::new(&path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.arg("-version");
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Checks if FFprobe is installed and functional on the operating system
pub fn is_ffprobe_available() -> bool {
    let path = find_ffprobe_path();
    if is_valid_executable(&path) {
        return true;
    }

    let mut cmd = Command::new(&path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.arg("-version");
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Locates the FFmpeg binary on the operating system
pub fn find_ffmpeg_path() -> PathBuf {
    if let Ok(read_guard) = CACHED_FFMPEG_PATH.read() {
        if let Some(ref path) = *read_guard {
            return path.clone();
        }
    }

    let resolved = resolve_binary("ffmpeg", "ffmpeg.exe");
    if let Ok(mut write_guard) = CACHED_FFMPEG_PATH.write() {
        *write_guard = Some(resolved.clone());
    }
    resolved
}

/// Locates the FFprobe binary on the operating system
pub fn find_ffprobe_path() -> PathBuf {
    if let Ok(read_guard) = CACHED_FFPROBE_PATH.read() {
        if let Some(ref path) = *read_guard {
            return path.clone();
        }
    }

    let resolved = resolve_binary("ffprobe", "ffprobe.exe");
    if let Ok(mut write_guard) = CACHED_FFPROBE_PATH.write() {
        *write_guard = Some(resolved.clone());
    }
    resolved
}

/// Constructs an FFmpeg `Command` with the real executable and safe flags
pub fn get_ffmpeg_cmd() -> Command {
    let path = find_ffmpeg_path();
    let mut cmd = Command::new(path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Constructs an FFprobe `Command` with the real executable and safe flags
pub fn get_ffprobe_cmd() -> Command {
    let path = find_ffprobe_path();
    let mut cmd = Command::new(path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

fn resolve_binary(base_name: &str, exe_name: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let app_data = std::env::var("APPDATA").unwrap_or_default();
        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();

        let mut candidate_paths: Vec<PathBuf> = Vec::new();

        // 1. HIGHEST PRIORITY: Binaries bundled with the application (Professional standard)
        if let Ok(curr_exe) = std::env::current_exe() {
            if let Some(parent) = curr_exe.parent() {
                candidate_paths.push(parent.join("resources").join("bin").join(exe_name));
                candidate_paths.push(parent.join("resources").join(exe_name));
                candidate_paths.push(parent.join("bin").join(exe_name));
                candidate_paths.push(parent.join(exe_name));
                candidate_paths.push(parent.join("_up_").join("resources").join("bin").join(exe_name));
                candidate_paths.push(parent.join("_up_").join("resources").join(exe_name));
            }
        }

        // 2. Local development environment (project root or src-tauri directory)
        if let Ok(cwd) = std::env::current_dir() {
            candidate_paths.push(cwd.join("src-tauri").join("bin").join(exe_name));
            candidate_paths.push(cwd.join("bin").join(exe_name));
        }

        // 3. Sniplic installation directories in the user profile
        if !local_app_data.is_empty() {
            candidate_paths.push(PathBuf::from(&local_app_data).join("Programs").join("Sniplic").join("resources").join("bin").join(exe_name));
            candidate_paths.push(PathBuf::from(&local_app_data).join("Programs").join("Sniplic").join("bin").join(exe_name));
            candidate_paths.push(PathBuf::from(&local_app_data).join("Sniplic").join("bin").join(exe_name));
            candidate_paths.push(PathBuf::from(&local_app_data).join("Sniplic").join("ffmpeg").join("bin").join(exe_name));
            candidate_paths.push(PathBuf::from(&local_app_data).join("com.sniplic.editor").join("bin").join(exe_name));
        }
        if !app_data.is_empty() {
            candidate_paths.push(PathBuf::from(&app_data).join("Sniplic").join("bin").join(exe_name));
            candidate_paths.push(PathBuf::from(&app_data).join("com.sniplic.editor").join("bin").join(exe_name));
        }

        for candidate in &candidate_paths {
            if is_valid_executable(candidate) {
                tracing::info!(binary = base_name, path = %candidate.display(), "Embedded/local FFmpeg binary successfully located");
                return candidate.clone();
            }
        }

        // 4. Fallback: Check if already available directly in global PATH
        let mut direct_test = Command::new(base_name);
        {
            use std::os::windows::process::CommandExt;
            direct_test.creation_flags(0x08000000);
        }
        direct_test.arg("-version");
        if let Ok(out) = direct_test.output() {
            if out.status.success() {
                return PathBuf::from(base_name);
            }
        }

        // 5. WinGet directories (links and packages)
        let mut fallback_paths: Vec<PathBuf> = Vec::new();
        if !local_app_data.is_empty() {
            let winget_links = PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join(exe_name);
            fallback_paths.push(winget_links);

            let winget_pkg_dir = PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if winget_pkg_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&winget_pkg_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let dir_name = entry.file_name().to_string_lossy().to_lowercase();
                            if dir_name.contains("ffmpeg") {
                                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                                    for sub in sub_entries.flatten() {
                                        let sub_path = sub.path();
                                        fallback_paths.push(sub_path.join("bin").join(exe_name));
                                        fallback_paths.push(sub_path.join(exe_name));
                                    }
                                }
                                fallback_paths.push(path.join("bin").join(exe_name));
                                fallback_paths.push(path.join(exe_name));
                            }
                        }
                    }
                }
            }
        }

        // 6. Standard Windows installation directories (Scoop, Chocolatey, Program Files, C:\ffmpeg)
        fallback_paths.push(PathBuf::from("C:\\ffmpeg\\bin").join(exe_name));
        fallback_paths.push(PathBuf::from("C:\\Program Files\\ffmpeg\\bin").join(exe_name));
        fallback_paths.push(PathBuf::from("C:\\Program Files (x86)\\ffmpeg\\bin").join(exe_name));
        fallback_paths.push(PathBuf::from("C:\\ProgramData\\chocolatey\\bin").join(exe_name));

        if !user_profile.is_empty() {
            fallback_paths.push(PathBuf::from(&user_profile).join("ffmpeg").join("bin").join(exe_name));
            fallback_paths.push(PathBuf::from(&user_profile).join("scoop").join("shims").join(exe_name));
            fallback_paths.push(PathBuf::from(&user_profile).join("scoop").join("apps").join("ffmpeg").join("current").join("bin").join(exe_name));
        }

        for candidate in fallback_paths {
            if is_valid_executable(&candidate) {
                tracing::info!(binary = base_name, path = %candidate.display(), "System FFmpeg binary successfully located");
                return candidate;
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // 1. Check in PATH
        let mut direct_test = Command::new(base_name);
        direct_test.arg("-version");
        if let Ok(out) = direct_test.output() {
            if out.status.success() {
                return PathBuf::from(base_name);
            }
        }

        // 2. Check standard Unix / Mac paths
        if let Ok(curr_exe) = std::env::current_exe() {
            if let Some(parent) = curr_exe.parent() {
                let cand = parent.join("resources").join("bin").join(exe_name);
                if is_valid_executable(&cand) {
                    return cand;
                }
                let cand = parent.join(exe_name);
                if is_valid_executable(&cand) {
                    return cand;
                }
            }
        }
    }

    PathBuf::from(base_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_and_ffprobe_resolution() {
        reset_cached_paths();
        assert!(is_ffmpeg_available(), "FFmpeg must be available on the system or bundled");
        assert!(is_ffprobe_available(), "FFprobe must be available on the system or bundled");

        let ffmpeg_path = find_ffmpeg_path();
        assert!(is_valid_executable(&ffmpeg_path), "FFmpeg path must be valid: {:?}", ffmpeg_path);

        let ffprobe_path = find_ffprobe_path();
        assert!(is_valid_executable(&ffprobe_path), "FFprobe path must be valid: {:?}", ffprobe_path);
    }
}
