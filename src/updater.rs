use crate::error::{Result, RsqlError};
use colored::Colorize;
use serde::Deserialize;
use std::env;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub const GITHUB_REPO: &str = "AMSComm/RustSQL";

#[derive(Deserialize, Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
}

/// Fetch latest release metadata from GitHub API
pub async fn check_latest_release() -> Result<Option<GithubRelease>> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let client = reqwest::Client::builder()
        .user_agent(format!("ruql/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| RsqlError::Other(e.to_string()))?;

    let res = client.get(&url).send().await;
    match res {
        Ok(resp) if resp.status().is_success() => {
            let release: GithubRelease = resp.json().await.map_err(|e| RsqlError::Other(e.to_string()))?;
            Ok(Some(release))
        }
        Ok(resp) if resp.status().as_u16() == 404 => Ok(None),
        Ok(resp) => Err(RsqlError::Other(format!("GitHub API returned HTTP status {}", resp.status()))),
        Err(e) => Err(RsqlError::Other(format!("Failed to connect to GitHub: {}", e))),
    }
}

/// Check if latest_tag (e.g. "v0.1.1") is newer than current_ver (e.g. "0.1.0")
pub fn is_newer_version(latest_tag: &str, current_ver: &str) -> bool {
    let clean_latest = latest_tag.trim_start_matches('v');
    let clean_current = current_ver.trim_start_matches('v');

    let parse_parts = |s: &str| -> Vec<u32> {
        s.split('.')
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    };

    let latest_parts = parse_parts(clean_latest);
    let current_parts = parse_parts(clean_current);

    latest_parts > current_parts
}

/// Return current platform target triple string
pub fn target_asset_name() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-unknown-linux-gnu";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc";

    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    return "unknown";
}

fn find_binary_in_dir(dir: &Path, target_bin: &str) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    if name == target_bin {
                        return Some(path);
                    }
                }
            } else if path.is_dir() {
                if let Some(found) = find_binary_in_dir(&path, target_bin) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Interactive update command (ruql --update)
pub async fn update_cli() -> Result<()> {
    println!("Checking for updates from https://github.com/{}...", GITHUB_REPO);
    let current_version = env!("CARGO_PKG_VERSION");
    let release = match check_latest_release().await? {
        Some(r) => r,
        None => {
            println!("No published releases found on repository {} yet.", GITHUB_REPO);
            return Ok(());
        }
    };

    if !is_newer_version(&release.tag_name, current_version) {
        println!("{}", format!("ruql is already up to date (version {}).", current_version).green().bold());
        return Ok(());
    }

    println!("{}", format!("Found new version: {} (current: {})", release.tag_name, current_version).cyan().bold());
    let target = target_asset_name();

    // Find matching asset
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(target))
        .ok_or_else(|| {
            RsqlError::Other(format!(
                "No release asset found matching your architecture '{}'. Available assets: {:?}",
                target,
                release.assets.iter().map(|a| &a.name).collect::<Vec<_>>()
            ))
        })?;

    println!("Downloading {}...", asset.name);
    let client = reqwest::Client::builder()
        .user_agent(format!("ruql/{}", current_version))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| RsqlError::Other(e.to_string()))?;

    let resp = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| RsqlError::Other(format!("Download failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(RsqlError::Other(format!("Failed to download asset: HTTP {}", resp.status())));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| RsqlError::Other(format!("Failed to read asset bytes: {}", e)))?;

    // Unpack archive into temporary directory
    let temp_dir = std::env::temp_dir().join(format!("ruql_update_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir)?;

    let is_windows = cfg!(target_os = "windows");
    let bin_filename = if is_windows { "ruql.exe" } else { "ruql" };

    let binary_path = if asset.name.ends_with(".tar.gz") || asset.name.ends_with(".tgz") {
        let tar_gz = flate2::read::GzDecoder::new(Cursor::new(bytes));
        let mut archive = tar::Archive::new(tar_gz);
        archive.unpack(&temp_dir)?;

        let direct = temp_dir.join(bin_filename);
        if direct.exists() {
            direct
        } else {
            find_binary_in_dir(&temp_dir, bin_filename).ok_or_else(|| {
                RsqlError::Other(format!("Could not locate '{}' in extracted archive", bin_filename))
            })?
        }
    } else if asset.name.ends_with(".zip") {
        let mut zip_archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| RsqlError::Other(format!("Failed to read zip archive: {}", e)))?;
        zip_archive
            .extract(&temp_dir)
            .map_err(|e| RsqlError::Other(format!("Failed to extract zip archive: {}", e)))?;

        let direct = temp_dir.join(bin_filename);
        if direct.exists() {
            direct
        } else {
            find_binary_in_dir(&temp_dir, bin_filename).ok_or_else(|| {
                RsqlError::Other(format!("Could not locate '{}' in extracted zip archive", bin_filename))
            })?
        }
    } else {
        return Err(RsqlError::Other(format!("Unsupported archive format: {}", asset.name)));
    };

    // Ensure executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&binary_path, perms);
    }

    println!("Installing updated binary...");
    self_replace::self_replace(&binary_path)
        .map_err(|e| RsqlError::Other(format!("Failed to replace binary: {}", e)))?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("{}", format!("Successfully updated ruql to {}!", release.tag_name).green().bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("v1.0.0", "0.9.9"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.1"));
        assert!(!is_newer_version("v0.0.9", "0.1.0"));
    }
}
