use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;

pub const REPOSITORY: &str = "siarheisinelnikau/desktop-organizer";
pub const INSTALLED: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub digest: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}
#[derive(Clone, Debug)]
pub struct Available {
    pub version: Version,
    pub asset: Asset,
}

pub fn select(release: Release, installed: &str) -> Result<Option<Available>> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = Version::parse(release.tag_name.trim_start_matches('v'))
        .context("Invalid release version")?;
    if !version.pre.is_empty() || version <= Version::parse(installed)? {
        return Ok(None);
    }
    let expected = format!("DesktopOrganizer-v{version}.exe");
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name == expected)
        .context("A newer release exists, but has no compatible Windows x64 portable executable")?;
    validate_asset(&asset, &version)?;
    Ok(Some(Available { version, asset }))
}

fn validate_asset(asset: &Asset, version: &Version) -> Result<()> {
    let url = Url::parse(&asset.browser_download_url)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Untrusted release asset URL"
    );
    ensure!(
        url.path()
            == format!(
                "/{REPOSITORY}/releases/download/v{version}/DesktopOrganizer-v{version}.exe"
            ),
        "Asset does not belong to the canonical release"
    );
    ensure!(
        asset.size > 0 && asset.size <= MAX_DOWNLOAD,
        "Release executable size outside limits"
    );
    Ok(())
}

pub fn check() -> Result<Option<Available>> {
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .new_agent();
    let response = agent
        .get(&format!(
            "https://api.github.com/repos/{REPOSITORY}/releases/latest"
        ))
        .header("User-Agent", "DesktopOrganizer/0.1.0")
        .header("Accept", "application/vnd.github+json")
        .call();
    let mut response = match response {
        Err(ureq::Error::StatusCode(404)) => {
            bail!("No published release is available at the planned repository yet (HTTP 404).")
        }
        Err(ureq::Error::StatusCode(403 | 429)) => {
            bail!("GitHub refused or rate-limited the update check. Try again later.")
        }
        other => {
            other.context("Could not check GitHub releases. Check your connection and retry.")?
        }
    };
    let release = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json::<Release>()
        .context("Invalid release response")?;
    select(release, INSTALLED)
}

pub fn download(
    update: &Available,
    directory: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    validate_asset(&update.asset, &update.version)?;
    ensure!(
        update.version > Version::parse(INSTALLED)?,
        "Update is not newer"
    );
    let digest = update.asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).context("This release has no GitHub SHA-256 digest. Installation is blocked until a verified asset is published.")?;
    ensure!(
        digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid release checksum"
    );
    fs::create_dir_all(directory)?;
    crate::operations::checked_path(directory, true)?;
    let path = directory.join(format!("DesktopOrganizer-v{}.download", update.version));
    // An old partial download is intentionally never executed or reused.
    let partial = path.with_extension(format!("{}.partial", std::process::id()));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .context("A partial update already exists; remove it manually before retrying")?;
    let result = (|| -> Result<()> {
        let agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(5)
            .timeout_global(Some(Duration::from_secs(180)))
            .build()
            .new_agent();
        let mut response = agent
            .get(&update.asset.browser_download_url)
            .header("User-Agent", "DesktopOrganizer/0.1.0")
            .call()
            .context("Update download failed")?;
        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0; 64 * 1024];
        let mut count = 0u64;
        let mut last_percent = 101;
        let mut hash = Sha256::new();
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            ensure!(
                count <= update.asset.size && count <= MAX_DOWNLOAD,
                "Update download exceeded expected size"
            );
            output.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
            let percent = count * 100 / update.asset.size;
            if last_percent != percent {
                progress(count, update.asset.size);
                last_percent = percent;
            }
        }
        ensure!(count == update.asset.size, "Incomplete update download");
        ensure!(
            format!("{:x}", hash.finalize()).eq_ignore_ascii_case(digest),
            "Update checksum mismatch; existing installation was preserved"
        );
        output.sync_all()?;
        Ok(())
    })();
    drop(output);
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    ensure!(
        !path.exists(),
        "A verified update is already staged. Restart or remove the old staged file first."
    );
    fs::rename(&partial, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: format!("DesktopOrganizer-{tag}.exe"),
                browser_download_url: format!(
                    "https://github.com/{REPOSITORY}/releases/download/{tag}/DesktopOrganizer-{tag}.exe"
                ),
                size: 100,
                digest: None,
            }],
        }
    }
    #[test]
    fn compares_semver_and_selects_exact_asset() {
        assert!(select(release("v0.10.0"), "0.9.0").unwrap().is_some());
        assert!(select(release("v0.1.0"), "0.1.0").unwrap().is_none());
        let mut bad = release("v0.2.0");
        bad.assets[0].browser_download_url = "https://example.com/evil.exe".into();
        assert!(select(bad, "0.1.0").is_err());
        let mut pre = release("v0.2.0");
        pre.prerelease = true;
        assert!(select(pre, "0.1.0").unwrap().is_none());
    }
}
