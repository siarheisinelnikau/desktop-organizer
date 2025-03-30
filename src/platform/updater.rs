use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_OBJECT_0},
    Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};

pub fn schedule(stage: &Path, digest: &str) -> Result<()> {
    let executable = std::env::current_exe()?;
    let helper = stage
        .parent()
        .context("Missing update directory")?
        .join(format!("update-helper-{}.exe", std::process::id()));
    ensure!(
        !helper.exists(),
        "An update helper already exists; restart and retry"
    );
    fs::copy(&executable, &helper)?;
    Command::new(&helper)
        .arg("--apply-update")
        .arg(std::process::id().to_string())
        .arg(&executable)
        .arg(stage)
        .arg(digest)
        .spawn()
        .context("Cannot start update helper; application remains unchanged")?;
    Ok(())
}

pub fn handle_helper_arguments() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != "--apply-update") {
        return false;
    }
    let result = (|| -> Result<()> {
        ensure!(args.len() == 6, "Invalid update helper arguments");
        let pid = args[2]
            .to_str()
            .context("Invalid process")?
            .parse::<u32>()?;
        let target = PathBuf::from(&args[3]);
        let stage = PathBuf::from(&args[4]);
        let digest = args[5].to_str().context("Invalid digest")?;
        crate::operations::checked_path(&target, true)?;
        crate::operations::checked_path(&stage, true)?;
        ensure!(
            target
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe")),
            "Update target is not executable"
        );
        let data = crate::persistence::default_data_dir()?.join("updates");
        ensure!(
            stage.parent() == Some(data.as_path())
                && stage.extension().is_some_and(|e| e == "download"),
            "Invalid update staging path"
        );
        let mut file = fs::File::open(&stage)?;
        ensure!(
            crate::operations::fingerprint(&mut file)?
                .sha256
                .eq_ignore_ascii_case(digest),
            "Staged update changed"
        );
        drop(file);
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if !process.is_null() {
            let status = unsafe { WaitForSingleObject(process, 120_000) };
            unsafe {
                CloseHandle(process);
            }
            ensure!(
                status == WAIT_OBJECT_0,
                "Timed out waiting for the application to exit"
            );
        }
        replace_verified(&target, &stage, digest)?;
        Command::new(target)
            .spawn()
            .context("Update installed; restart DesktopOrganizer manually")?;
        Ok(())
    })();
    if let Err(error) = result {
        // A noninteractive helper reports persistent errors to be shown on the next app launch.
        if let Ok(data) = crate::persistence::default_data_dir() {
            let _ = fs::write(data.join("update-error.txt"), format!("{error:#}"));
        }
    }
    true
}

/// Prepare beside the executable, verify and flush before a single atomic replacement.
/// A process/power failure during preparation leaves the installed executable intact.
fn replace_verified(target: &Path, stage: &Path, digest: &str) -> Result<()> {
    crate::operations::checked_path(target, true)?;
    crate::operations::checked_path(stage, true)?;
    let incoming = target.with_extension("exe.incoming");
    let backup = target.with_extension("exe.previous");
    ensure!(
        !backup.exists(),
        "An executable backup exists; preserve it and resolve the earlier update first"
    );
    let mut reader = fs::File::open(stage)?;
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&incoming)
        .context(
            "Cannot prepare an update beside this executable; choose a writable portable location",
        )?;
    let prepared = (|| -> Result<()> {
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n])?;
        }
        writer.sync_all()?;
        ensure!(
            crate::operations::fingerprint(&mut fs::File::open(&incoming)?)?
                .sha256
                .eq_ignore_ascii_case(digest),
            "Prepared executable checksum mismatch"
        );
        Ok(())
    })();
    drop(writer);
    if let Err(error) = prepared {
        let _ = fs::remove_file(&incoming);
        return Err(error);
    }
    fs::copy(target, &backup).context("Cannot back up the current executable")?;
    fs::OpenOptions::new()
        .write(true)
        .open(&backup)?
        .sync_all()?;
    let src: Vec<_> = incoming.as_os_str().encode_wide().chain(Some(0)).collect();
    let dst: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        MoveFileExW(
            src.as_ptr(),
            dst.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        let error = std::io::Error::last_os_error();
        let _ = fs::remove_file(incoming);
        let _ = fs::remove_file(backup);
        return Err(error).context("Atomic update failed; the current executable is unchanged");
    }
    let _ = fs::remove_file(stage);
    let _ = fs::remove_file(backup);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_or_invalid_update_never_replaces_installed_bytes() {
        fs::create_dir_all(".test-output").unwrap();
        let temp =
            tempfile::tempdir_in(std::env::current_dir().unwrap().join(".test-output")).unwrap();
        let target = temp.path().join("app.exe");
        let stage = temp.path().join("new.download");
        fs::write(&target, b"working executable fixture").unwrap();
        fs::write(&stage, b"incomplete download fixture").unwrap();
        assert!(replace_verified(&target, &stage, &"0".repeat(64)).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"working executable fixture");
        let digest = crate::operations::fingerprint(&mut fs::File::open(&stage).unwrap())
            .unwrap()
            .sha256;
        replace_verified(&target, &stage, &digest).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"incomplete download fixture");
        assert!(!stage.exists());
    }
}
