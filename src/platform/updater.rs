//! Internal shared-updater protocol, dispatched before the normal application starts.
pub fn handle_helper_arguments() -> bool {
    if std::env::args_os()
        .nth(1)
        .is_none_or(|arg| arg != "--apply-update")
    {
        return false;
    }
    let result =
        crate::updates::config().and_then(|config| rust_auto_updator::apply_update(&config));
    if let Err(error) = result {
        if let Ok(data) = crate::persistence::default_data_dir() {
            let _ = std::fs::write(data.join("update-error.txt"), error);
        }
        std::process::exit(1);
    }
    true
}

pub fn cleanup_after_startup() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "--cleanup-update")
        && let Some(directory) = args.get(2)
        && let Ok(config) = crate::updates::config()
    {
        rust_auto_updator::cleanup_after_update(directory.into(), config.install);
    }
}
