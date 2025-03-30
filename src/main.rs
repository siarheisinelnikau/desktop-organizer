#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(all(windows, not(target_arch = "x86_64")))]
compile_error!("The initial DesktopOrganizer release supports Windows x64 only.");

#[cfg(windows)]
fn main() {
    desktop_organizer::platform::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("DesktopOrganizer requires Windows 10 or 11. The core library tests are portable.");
}
