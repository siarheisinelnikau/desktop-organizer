//! DesktopOrganizer product configuration; all update mechanics live in the shared core.
use anyhow::Result;
use rust_auto_updator::Config;
pub use rust_auto_updator::{Prepared, Release as Available};

pub const REPOSITORY: &str = "siarheisinelnikau/desktop-organizer";
pub const INSTALLED: &str = env!("CARGO_PKG_VERSION");

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

pub fn check() -> Result<Option<Available>> {
    rust_auto_updator::check_latest(&config().map_err(anyhow::Error::msg)?)
        .map_err(anyhow::Error::msg)
}

pub fn download(update: &Available, progress: impl FnMut(u64, u64)) -> Result<Prepared> {
    rust_auto_updator::prepare(update, &config().map_err(anyhow::Error::msg)?, progress)
        .map_err(anyhow::Error::msg)
}

pub fn install(mut prepared: Prepared) -> Result<()> {
    rust_auto_updator::handoff(&mut prepared).map_err(anyhow::Error::msg)
}
