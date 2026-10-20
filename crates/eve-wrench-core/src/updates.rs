//! Fork release discovery and verified portable downloads. Blocking: use a worker.
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use crate::esi::AGENT;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const GITHUB_REPO: &str = "khivus/WrenchAppRe";
pub const RELEASES_URL: &str = "https://github.com/khivus/WrenchAppRe/releases";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub asset: Option<UpdateAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ReleasePayload {
    List(Vec<Release>),
    Single(Release),
}

fn version(text: &str) -> Option<Version> {
    Version::parse(text.trim_start_matches('v')).ok()
}

pub fn is_preview_version(text: &str) -> bool {
    version(text).is_some_and(|v| !v.pre.is_empty())
}

fn trusted_asset_url(url: &str) -> bool {
    url.starts_with("https://github.com/khivus/WrenchAppRe/releases/download/")
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit())
}

fn select_update(payload: ReleasePayload, current: &str, windows_x64: bool) -> Option<UpdateInfo> {
    let current_version = version(current)?;
    let previews = !current_version.pre.is_empty();
    let releases = match payload {
        ReleasePayload::List(list) => list,
        ReleasePayload::Single(release) => vec![release],
    };
    let (release, latest) = releases
        .into_iter()
        .filter_map(|release| {
            let latest = version(&release.tag_name)?;
            (!release.draft
                && (previews || (!release.prerelease && latest.pre.is_empty()))
                && latest.cmp_precedence(&current_version).is_gt()
                && release
                    .html_url
                    .starts_with("https://github.com/khivus/WrenchAppRe/releases/"))
            .then_some((release, latest))
        })
        .max_by(|(_, a), (_, b)| a.cmp_precedence(b))?;
    let expected_name = format!("EVE-Wrench_{latest}_x64-portable.exe");
    let asset = windows_x64
        .then(|| {
            release.assets.iter().find_map(|asset| {
                let hash = asset.digest.as_deref()?.strip_prefix("sha256:")?;
                (asset.name == expected_name
                    && trusted_asset_url(&asset.browser_download_url)
                    && valid_hash(hash)
                    && asset.size > 0
                    && asset.size <= MAX_DOWNLOAD)
                    .then(|| UpdateAsset {
                        name: asset.name.clone(),
                        url: asset.browser_download_url.clone(),
                        size: asset.size,
                        sha256: hash.to_ascii_lowercase(),
                    })
            })
        })
        .flatten();
    Some(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        release_url: release.html_url,
        release_notes: release.body.unwrap_or_default(),
        asset,
    })
}

pub fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let endpoint = if is_preview_version(APP_VERSION) {
        "?per_page=100"
    } else {
        "/latest"
    };
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases{endpoint}");
    let response = AGENT
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call();
    let mut response = match response {
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(format!("Failed to check releases: {error}")),
        Ok(response) => response,
    };
    let payload = response
        .body_mut()
        .read_json()
        .map_err(|error| format!("Invalid release response: {error}"))?;
    Ok(select_update(
        payload,
        APP_VERSION,
        cfg!(all(windows, target_arch = "x86_64")),
    ))
}

/// Streams to a new file and removes incomplete, mismatched or invalid downloads.
pub fn download_update(asset: &UpdateAsset, destination: &Path) -> Result<(), String> {
    if !trusted_asset_url(&asset.url)
        || !valid_hash(&asset.sha256)
        || asset.size == 0
        || asset.size > MAX_DOWNLOAD
    {
        return Err("The release has no verified portable Windows download".into());
    }
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .user_agent(concat!("EVE-Wrench/", env!("CARGO_PKG_VERSION")))
            .timeout_global(Some(Duration::from_secs(600)))
            .build(),
    );
    let mut response = agent
        .get(&asset.url)
        .call()
        .map_err(|error| format!("Download failed: {error}"))?;
    save_download(&mut response.body_mut().as_reader(), destination, asset)
}

fn save_download(
    reader: &mut impl Read,
    destination: &Path,
    asset: &UpdateAsset,
) -> Result<(), String> {
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| format!("Cannot create update file: {error}"))?;
    let result = (|| {
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > asset.size {
                return Err("Download exceeds the release asset size".into());
            }
            hash.update(&buffer[..count]);
            file.write_all(&buffer[..count])
                .map_err(|e| e.to_string())?;
        }
        if total != asset.size
            || format!("{:x}", hash.finalize()) != asset.sha256.to_ascii_lowercase()
        {
            return Err("Update size or SHA-256 does not match the GitHub release".into());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        verify_windows_exe(destination, &asset.sha256)
    })();
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

pub fn verify_windows_exe(path: &Path, expected_hash: &str) -> Result<(), String> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header)
        .map_err(|_| "Incomplete Windows executable")?;
    if &header[..2] != b"MZ" {
        return Err("Update is not a Windows executable".into());
    }
    let offset = u32::from_le_bytes(header[60..64].try_into().unwrap()) as u64;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut pe = [0u8; 24];
    file.read_exact(&mut pe)
        .map_err(|_| "Incomplete PE header")?;
    if &pe[..4] != b"PE\0\0"
        || u16::from_le_bytes([pe[4], pe[5]]) != 0x8664
        || u16::from_le_bytes([pe[22], pe[23]]) & 0x2000 != 0
    {
        return Err("Update is not an x64 Windows application".into());
    }
    file.rewind().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
    if !valid_hash(expected_hash)
        || format!("{:x}", hash.finalize()) != expected_hash.to_ascii_lowercase()
    {
        return Err("Staged update failed SHA-256 verification".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
