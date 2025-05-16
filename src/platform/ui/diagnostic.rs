//! Bounded native integration/load check. No input injection or desktop capture.
use super::*;
use std::time::{Duration, Instant};

fn counters() -> [u64; 4] {
    [
        MAIN_PAINTS.load(Ordering::Relaxed),
        GROUP_PAINTS.load(Ordering::Relaxed),
        LAYOUTS.load(Ordering::Relaxed),
        OWNER_DRAWS.load(Ordering::Relaxed),
    ]
}
unsafe fn cpu() -> Result<u64> {
    unsafe {
        let (mut creation, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        ensure!(
            GetProcessTimes(
                GetCurrentProcess(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user
            ) != 0,
            "Cannot sample CPU"
        );
        Ok(
            ((kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64)
                + ((user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64),
        )
    }
}
unsafe fn pump(duration: Duration) -> Result<()> {
    unsafe {
        let end = Instant::now() + duration;
        let mut msg: MSG = zeroed();
        while Instant::now() < end {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                ensure!(msg.message != WM_QUIT, "Unexpected quit during diagnostic");
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let timeout = end
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(1000) as u32;
            if timeout > 0 {
                MsgWaitForMultipleObjectsEx(0, null(), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
        }
        Ok(())
    }
}
unsafe fn sample(state: &str) -> Result<serde_json::Value> {
    unsafe {
        pump(Duration::from_secs(1))?;
        let start = Instant::now();
        let before = counters();
        let cpu_before = cpu()?;
        pump(Duration::from_secs(5))?;
        let seconds = start.elapsed().as_secs_f64();
        let cpu_seconds = (cpu()? - cpu_before) as f64 / 10_000_000.0;
        let delta: Vec<_> = counters()
            .into_iter()
            .zip(before)
            .map(|(a, b)| a - b)
            .collect();
        Ok(
            serde_json::json!({"state":state,"seconds":seconds,"cpu_seconds":cpu_seconds,
        "cpu_one_core_percent":100.0*cpu_seconds/seconds,
        "cpu_machine_percent":100.0*cpu_seconds/seconds/std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1) as f64,
        "main_rasters":delta[0],"group_rasters":delta[1],"layouts":delta[2],"owner_draws":delta[3]}),
        )
    }
}
pub(super) unsafe fn run(options: &capture::Options, hwnd: HWND) -> Result<()> {
    unsafe {
        APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            app.hidden = false;
            app.show_groups();
            app.register_tray();
        });
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let visible = sample("visible settled")?;
        let tray_registered = APP.with(|cell| cell.borrow().as_ref().unwrap().tray_available);
        // Exercise the OS's SC_CLOSE -> WM_CLOSE reentry, not a test-only hide shortcut.
        SendMessageW(hwnd, WM_SYSCOMMAND, SC_CLOSE as usize, 0);
        let close_preserved_window = IsWindow(hwnd) != 0;
        let close_hid_to_tray = close_preserved_window && IsWindowVisible(hwnd) == 0;
        // A sandbox without Shell access can refuse the tray. Hidden rendering is still measurable.
        ShowWindow(hwnd, SW_HIDE);
        APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            app.hidden = true;
            app.show_groups();
        });
        let hidden = sample("hidden settled")?;
        SendMessageW(hwnd, RESTORE, 0, 0);
        APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            app.hidden = false;
            app.show_groups();
        });
        let restored_visible = IsWindowVisible(hwnd) != 0;
        let restored = sample("restored settled")?;
        let report = serde_json::json!({"pid":std::process::id(),"executable":std::env::current_exe()?,"profile":"release",
        "backend":"CPU GDI / native Win32 controls; no GPU renderer","fixture_items":APP.with(|c|c.borrow().as_ref().unwrap().layout.items.len()),
        "tray_registered":tray_registered,"system_close_preserved_window":close_preserved_window,"system_close_hid_to_tray":close_hid_to_tray,
        "restore_visible":restored_visible,"samples":[visible,hidden,restored],
        "gpu_telemetry":"not collected; compositor/driver timing is not certified",
        "scope":"isolated in-process integration and idle load; physical tray clicks and keyboard traversal not tested"});
        fs::create_dir_all(options.output.parent().context("Missing report parent")?)?;
        fs::write(&options.output, serde_json::to_vec_pretty(&report)?)?;
        ensure!(
            close_preserved_window && restored_visible,
            "Native close/restore regression"
        );
        Ok(())
    }
}
