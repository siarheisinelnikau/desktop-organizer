#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() { desktop_organizer::platform::run(); }

#[cfg(not(windows))]
fn main() { eprintln!("DesktopOrganizer requires Windows 10 or 11. The core library tests are portable."); }
