use super::*;
use serde_json::{Value, json};

fn release(tag: &str) -> Value {
    json!({
        "tag_name": tag,
        "html_url": format!("{RELEASES_URL}/tag/{tag}"),
        "assets": [{
            "name": format!("EVE-Wrench_{}_x64-portable.exe", tag.trim_start_matches('v')),
            "size": 128,
            "digest": format!("sha256:{}", "a".repeat(64)),
            "browser_download_url": format!("{RELEASES_URL}/download/{tag}/app.exe")
        }]
    })
}

fn select(value: Value, current: &str) -> Option<UpdateInfo> {
    select_update(serde_json::from_value(value).unwrap(), current, true)
}

#[test]
fn release_selection_respects_versions_channels_and_the_fork() {
    assert!(select(release("v0.5.1"), "0.5.1").is_none());
    assert!(select(release("v0.4.1"), "0.5.1").is_none());
    assert!(select(release("v0.5.1+new-build"), "0.5.1+old-build").is_none());
    assert!(select(release("broken-tag"), "0.5.1").is_none());
    assert!(!is_preview_version("0.5.1"));
    assert!(is_preview_version("0.6.0-preview.2"));
    let mut draft = release("v9.0.0");
    draft["draft"] = json!(true);
    let mut preview = release("v0.6.0-preview.10");
    preview["prerelease"] = json!(true);
    let list = json!([draft, preview, release("v0.5.2"), release("v0.5.1")]);
    assert_eq!(
        select(list.clone(), "0.5.1").unwrap().latest_version,
        "0.5.2"
    );
    assert_eq!(
        select(list.clone(), "0.6.0-preview.2")
            .unwrap()
            .latest_version,
        "0.6.0-preview.10"
    );
    assert_eq!(
        select(json!([release("v0.6.0"), list[1]]), "0.6.0-preview.2")
            .unwrap()
            .latest_version,
        "0.6.0"
    );
    let mut upstream = release("v9.0.0");
    upstream["html_url"] =
        json!("https://github.com/eve-wrench/eve-wrench-app/releases/tag/v9.0.0");
    assert!(select(upstream, "0.5.1").is_none());
}

#[test]
fn only_verified_matching_portable_assets_can_self_install() {
    assert!(select(release("v0.5.2"), "0.5.1").unwrap().asset.is_some());
    for (field, value) in [
        ("digest", Value::Null),
        ("digest", json!("sha256:invalid")),
        (
            "browser_download_url",
            json!("https://example.com/evil.exe"),
        ),
        (
            "browser_download_url",
            json!("http://github.com/khivus/WrenchAppRe/releases/download/v0.5.2/app.exe"),
        ),
        ("name", json!("EVE-Wrench_0.5.2_x64-setup.exe")),
        ("size", json!(MAX_DOWNLOAD + 1)),
    ] {
        let mut value_json = release("v0.5.2");
        value_json["assets"][0][field] = value;
        assert!(
            select(value_json, "0.5.1").unwrap().asset.is_none(),
            "{field}"
        );
    }
}

fn fixture_exe() -> Vec<u8> {
    let mut bytes = vec![0u8; 256];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes
}

#[test]
fn streamed_downloads_validate_hash_size_and_pe_and_remove_failures() {
    let dir = std::env::temp_dir().join(format!(
        "wrench-download-test-{}-{}",
        std::process::id(),
        crate::marshal::filetime_now()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("new.exe");
    let bytes = fixture_exe();
    let asset = UpdateAsset {
        name: "fixture.exe".into(),
        url: String::new(),
        size: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    save_download(&mut bytes.as_slice(), &path, &asset).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    // Existing files are never truncated, even when a download is retried.
    assert!(save_download(&mut bytes.as_slice(), &path, &asset).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::remove_file(&path).unwrap();
    for bad_bytes in [bytes[..32].to_vec(), vec![0u8; 256], vec![1u8; 257]] {
        assert!(save_download(&mut bad_bytes.as_slice(), &path, &asset).is_err());
        assert!(!path.exists());
    }
    // A valid hash of a non-executable must still fail validation.
    let bad = vec![1u8; 256];
    let mut bad_asset = asset.clone();
    bad_asset.sha256 = format!("{:x}", Sha256::digest(&bad));
    assert!(save_download(&mut bad.as_slice(), &path, &bad_asset).is_err());
    assert!(!path.exists());
    let resolved = dir.canonicalize().unwrap();
    assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    fs::remove_dir_all(resolved).unwrap();
}

#[test]
#[ignore = "Downloads and verifies the current public fork release; run explicitly with network access"]
fn live_fork_release_asset_verifies() {
    let payload = AGENT
        .get(&format!(
            "https://api.github.com/repos/{GITHUB_REPO}/releases/latest"
        ))
        .call()
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap();
    let info = select_update(payload, "0.0.0", true).unwrap();
    let asset = info.asset.unwrap();
    let dir = std::env::temp_dir().join(format!("wrench-live-download-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let file = dir.join("release.exe");
    let result = download_update(&asset, &file);
    let _ = fs::remove_file(&file);
    fs::remove_dir(&dir).unwrap();
    result.unwrap();
}
