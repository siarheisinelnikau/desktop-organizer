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
        let (base, key, auto_check, note, data) = APP.with(|cell| {
            let borrow = cell.borrow();
            let app = borrow.as_ref().unwrap();
            (
                app.c(BASE),
                app.c(KEY),
                app.c(AUTO_CHECK),
                app.c(AI_TRANSPORT_NOTE),
                app.data.clone(),
            )
        });
        // Only synthetic data is saved in this diagnostic's explicit output directory.
        // Use native notifications/Save settings, without connecting to this example address.
        SetWindowTextW(base, w!("http://192.0.2.10:8080/v1"));
        let http_note = GetWindowTextLengthW(note) > 0;
        SetWindowTextW(base, w!("https://example.test/v1"));
        let https_note_hidden = GetWindowTextLengthW(note) == 0;
        SetWindowTextW(base, w!("http://192.0.2.10:8080/v1"));
        let synthetic_token = format!("synthetic-diagnostic-{}", std::process::id());
        SetWindowTextW(key, wide(&synthetic_token).as_ptr());
        SendMessageW(auto_check, BM_SETCHECK, BST_UNCHECKED as _, 0);
        SendMessageW(hwnd, WM_COMMAND, SAVE_SETTINGS as usize, 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        let settings_persisted = loop {
            pump(Duration::from_millis(50))?;
            if let Ok(layout) = persistence::load::<Layout>(&data.join("layout.json")) {
                let mut plaintext =
                    super::super::secrets::unprotect(&layout.settings.protected_api_key)?;
                let correct = layout.settings.ai_base_url == "http://192.0.2.10:8080/v1"
                    && !layout.settings.startup_update_check
                    && plaintext == synthetic_token.as_bytes();
                plaintext.fill(0);
                if correct {
                    break true;
                }
            }
            if Instant::now() >= deadline {
                break false;
            }
        };
        let token_field_cleared = GetWindowTextLengthW(key) == 0;
        ensure!(
            http_note && https_note_hidden && settings_persisted && token_field_cleared,
            "Native HTTP settings save regression"
        );
        let settings_check = serde_json::json!({"http_warning_shown":http_note,"https_warning_hidden":https_note_hidden,"http_url_and_startup_opt_out_persisted":settings_persisted,"synthetic_token_dpapi_roundtrip":settings_persisted,"token_field_cleared":token_field_cleared,"live_provider_requests":0});
        APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            app.status("");
            app.hidden = false;
            app.show_groups();
            app.register_tray();
        });
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let visible = sample("visible settled")?;
        let fonts_before = FONT_CREATIONS.load(Ordering::Relaxed);
        let layouts_before = LAYOUTS.load(Ordering::Relaxed);
        let mut max_dispatch_ms = 0f64;
        let mut wrong_visibility = 0u64;
        let mut paints_inside_dispatch = 0u64;
        for _ in 0..10 {
            for id in [PAGE_AI, PAGE_SETTINGS, PAGE_LIBRARY] {
                let before = MAIN_PAINTS.load(Ordering::Relaxed);
                let start = Instant::now();
                SendMessageW(hwnd, WM_COMMAND, id as usize, 0);
                max_dispatch_ms = max_dispatch_ms.max(start.elapsed().as_secs_f64() * 1000.0);
                paints_inside_dispatch += MAIN_PAINTS.load(Ordering::Relaxed) - before;
                pump(Duration::from_millis(20))?;
                APP.with(|cell| {
                    let mut borrow = cell.borrow_mut();
                    let app = borrow.as_mut().unwrap();
                    // Repopulating the library must not make another page's list visible.
                    app.refresh();
                    for control in &app.controls {
                        if (GetWindowLongPtrW(control.hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0)
                            != app.control_visible(control)
                        {
                            wrong_visibility += 1;
                        }
                    }
                });
            }
        }
        let transitions = serde_json::json!({"count":30,"fonts_created":FONT_CREATIONS.load(Ordering::Relaxed)-fonts_before,"layouts":LAYOUTS.load(Ordering::Relaxed)-layouts_before,"unexpected_visibility":wrong_visibility,"scene_paints_during_dispatch":paints_inside_dispatch,"max_dispatch_ms":max_dispatch_ms,"scope":"native dispatch and visibility; not a subjective flicker or presentation-latency measurement"});
        let after_navigation = sample("after navigation settled")?;
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
        "restore_visible":restored_visible,"settings":settings_check,"navigation":transitions,"samples":[visible,after_navigation,hidden,restored],
        "gpu_telemetry":"not collected; compositor/driver timing is not certified",
        "scope":"isolated in-process integration and idle load; physical tray clicks and keyboard traversal not tested"});
        fs::create_dir_all(options.output.parent().context("Missing report parent")?)?;
        fs::write(&options.output, serde_json::to_vec_pretty(&report)?)?;
        ensure!(
            close_preserved_window && restored_visible,
            "Native close/restore regression"
        );
        ensure!(
            wrong_visibility == 0 && FONT_CREATIONS.load(Ordering::Relaxed) == fonts_before,
            "Navigation visibility or font recreation regression"
        );
        Ok(())
    }
}
