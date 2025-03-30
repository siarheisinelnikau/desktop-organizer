use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_STATE_BYTES: u64 = 8 * 1024 * 1024;

pub fn load<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_STATE_BYTES,
        "State file is too large"
    );
    let mut bytes = vec![];
    file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_STATE_BYTES,
        "State file is too large"
    );
    serde_json::from_slice(&bytes).context("Invalid JSON state; original file has been preserved")
}

pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    ensure!(
        bytes.len() as u64 <= MAX_STATE_BYTES,
        "State file is too large"
    );
    let parent = path.parent().context("Missing state directory")?;
    fs::create_dir_all(parent)?;
    let temp = path.with_extension("json.pending");
    // The application holds its instance lock. A leftover temporary file is safe to replace.
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let src: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                src.as_ptr(),
                dst.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error())
                .context("Cannot atomically replace JSON state");
        }
    }
    #[cfg(not(windows))]
    fs::rename(temp, path)?;
    Ok(())
}

pub fn default_data_dir() -> Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?;
    Ok(PathBuf::from(local).join("DesktopOrganizer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layout;
    #[test]
    fn atomic_replacement_roundtrips_layout_and_preserves_corrupt_input() {
        fs::create_dir_all(".test-output").unwrap();
        let temp =
            tempfile::tempdir_in(std::env::current_dir().unwrap().join(".test-output")).unwrap();
        let path = temp.path().join("layout.json");
        let mut layout = Layout::default();
        save(&path, &layout).unwrap();
        layout.settings.startup_update_check = false;
        layout.groups[0].collapsed = true;
        save(&path, &layout).unwrap();
        let restored: Layout = load(&path).unwrap();
        restored.validate().unwrap();
        assert!(!restored.settings.startup_update_check && restored.groups[0].collapsed);
        fs::write(&path, b"invalid JSON fixture").unwrap();
        assert!(load::<Layout>(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"invalid JSON fixture");
    }
}
