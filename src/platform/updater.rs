use anyhow::{Context, Result, ensure};
use std::{fs, path::{Path, PathBuf}, process::Command};
use windows_sys::Win32::{Foundation::{CloseHandle, WAIT_OBJECT_0}, System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE}};

pub fn schedule(stage: &Path, digest: &str) -> Result<()> {
    let executable = std::env::current_exe()?;
    let helper = stage.parent().context("Missing update directory")?.join(format!("update-helper-{}.exe", std::process::id()));
    ensure!(!helper.exists(), "An update helper already exists; restart and retry");
    fs::copy(&executable, &helper)?;
    Command::new(&helper).arg("--apply-update").arg(std::process::id().to_string()).arg(&executable).arg(stage).arg(digest).spawn().context("Cannot start update helper; application remains unchanged")?;
    Ok(())
}

pub fn handle_helper_arguments() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if !args.get(1).is_some_and(|a| a == "--apply-update") { return false; }
    let result = (|| -> Result<()> {
        ensure!(args.len() == 6, "Invalid update helper arguments");
        let pid = args[2].to_str().context("Invalid process")?.parse::<u32>()?;
        let target = PathBuf::from(&args[3]);
        let stage = PathBuf::from(&args[4]);
        let digest = args[5].to_str().context("Invalid digest")?;
        crate::operations::checked_path(&target, true)?;
        crate::operations::checked_path(&stage, true)?;
        ensure!(target.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")), "Update target is not executable");
        let data = crate::persistence::default_data_dir()?.join("updates");
        ensure!(stage.parent() == Some(data.as_path()) && stage.extension().is_some_and(|e| e == "download"), "Invalid update staging path");
        let mut file = fs::File::open(&stage)?;
        ensure!(crate::operations::fingerprint(&mut file)?.sha256.eq_ignore_ascii_case(digest), "Staged update changed");
        drop(file);
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if !process.is_null() {
            let status = unsafe { WaitForSingleObject(process, 120_000) };
            unsafe { CloseHandle(process); }
            ensure!(status == WAIT_OBJECT_0, "Timed out waiting for the application to exit");
        }
        let backup = target.with_extension("exe.previous");
        ensure!(!backup.exists(), "Previous executable backup already exists; preserve it and resolve manually");
        // Keep a complete backup until replacement has succeeded. Never touch the settings directory.
        fs::rename(&target, &backup).context("Cannot rename the old executable; it may be read-only")?;
        if let Err(error) = fs::copy(&stage, &target) {
            let _ = fs::remove_file(&target);
            let _ = fs::rename(&backup, &target);
            let _ = Command::new(&target).spawn();
            return Err(error).context("Install failed; restored previous executable");
        }
        let _ = fs::remove_file(stage);
        let _ = fs::remove_file(backup);
        Command::new(target).spawn().context("Update installed; restart DesktopOrganizer manually")?;
        Ok(())
    })();
    if let Err(error) = result {
        // A noninteractive helper reports persistent errors to be shown on the next app launch.
        if let Ok(data) = crate::persistence::default_data_dir() { let _ = fs::write(data.join("update-error.txt"), format!("{error:#}")); }
    }
    true
}
