//! DesktopOrganizer product configuration; all update mechanics live in the shared core.
use anyhow::Result;
use rust_auto_updator::Config;
pub use rust_auto_updator::{Prepared, Release as Available};

pub const REPOSITORY: &str = "siarheisinelnikau/desktop-organizer";
pub const INSTALLED: &str = env!("CARGO_PKG_VERSION");
pub const PUBLIC_RELEASE_UNAVAILABLE: &str = "Public releases are unavailable (GitHub 404). The repository may be private or have no published release.";

pub fn config() -> Result<Config, String> {
    let mut config = Config::new(
        "siarheisinelnikau",
        "desktop-organizer",
        INSTALLED,
        "DesktopOrganizer",
    )?;
    config.expected_subsystem = Some(2);
    config.checksum_name = "SHA256SUMS".into();
    config.max_executable_size = 128 * 1024 * 1024;
    config.install.stage_prefix = ".desktop-organizer-update-".into();
    Ok(config)
}

#[derive(Debug)]
pub enum CheckStatus {
    Available(Box<Available>),
    Current,
    PublicReleaseUnavailable,
}

fn classify_check(result: Result<Option<Available>, String>) -> Result<CheckStatus> {
    match result {
        Ok(Some(release)) => Ok(CheckStatus::Available(Box::new(release))),
        Ok(None) => Ok(CheckStatus::Current),
        // The shared core currently exposes transport failures as text, without a status type.
        // Match its exact pinned 404 error; never turn other failures into "up to date".
        Err(message) if message == "Could not contact GitHub: http status: 404" => {
            Ok(CheckStatus::PublicReleaseUnavailable)
        }
        Err(message) => Err(anyhow::Error::msg(message)),
    }
}

pub fn check() -> Result<CheckStatus> {
    classify_check(rust_auto_updator::check_latest(
        &config().map_err(anyhow::Error::msg)?,
    ))
}

pub fn download(update: &Available, progress: impl FnMut(u64, u64)) -> Result<Prepared> {
    rust_auto_updator::prepare(update, &config().map_err(anyhow::Error::msg)?, progress)
        .map_err(anyhow::Error::msg)
}

pub fn install(mut prepared: Prepared) -> Result<()> {
    rust_auto_updator::handoff(&mut prepared).map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_public_release_is_distinct_from_current_and_failed_checks() {
        let missing = format!("Could not contact GitHub: {}", ureq::Error::StatusCode(404));
        assert!(matches!(
            classify_check(Err(missing)),
            Ok(CheckStatus::PublicReleaseUnavailable)
        ));
        assert!(matches!(classify_check(Ok(None)), Ok(CheckStatus::Current)));
        for error in [
            "GitHub's request limit was reached. Try again later.",
            "Could not contact GitHub: timeout",
            "GitHub returned an invalid release: bad JSON",
        ] {
            assert!(classify_check(Err(error.into())).is_err());
        }
    }
}
