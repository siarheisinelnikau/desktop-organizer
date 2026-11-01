//! Bounded native integration/load check. No input injection or desktop capture.
use super::*;
use std::time::{Duration, Instant};
thread_local! { static MESSAGES: RefCell<std::collections::BTreeMap<(u8,u32),u64>> = const { RefCell::new(std::collections::BTreeMap::new()) }; }
static WAITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(super) fn note_message(source: u8, message: u32) {
    MESSAGES.with(|messages| *messages.borrow_mut().entry((source, message)).or_default() += 1);
}
unsafe fn ui_cpu() -> Result<u64> {
    unsafe {
        let (mut creation, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        ensure!(
            GetThreadTimes(
                GetCurrentThread(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user
            ) != 0,
            "Cannot sample UI thread CPU"
        );
        Ok(
            ((kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64)
                + ((user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64),
        )
    }
}

fn counters() -> [u64; 9] {
    [
        MAIN_PAINTS.load(Ordering::Relaxed),
        GROUP_PAINTS.load(Ordering::Relaxed),
        LAYOUTS.load(Ordering::Relaxed),
        OWNER_DRAWS.load(Ordering::Relaxed),
        MAIN_PAINT_MESSAGES.load(Ordering::Relaxed),
        MAIN_PRINT_MESSAGES.load(Ordering::Relaxed),
        REFRESHES.load(Ordering::Relaxed),
        STATUS_UPDATES.load(Ordering::Relaxed),
        MAIN_BLITS.load(Ordering::Relaxed),
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
                note_message(1, msg.message);
                ensure!(msg.message != WM_QUIT, "Unexpected quit during diagnostic");
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let timeout = end
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(1000) as u32;
            if timeout > 0 {
                WAITS.fetch_add(1, Ordering::Relaxed);
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
        let ui_before = ui_cpu()?;
        let waits_before = WAITS.load(Ordering::Relaxed);
        let messages_before = MESSAGES.with(|messages| messages.borrow().clone());
        pump(Duration::from_secs(5))?;
        let seconds = start.elapsed().as_secs_f64();
        let cpu_seconds = (cpu()? - cpu_before) as f64 / 10_000_000.0;
        let ui_seconds = (ui_cpu()? - ui_before) as f64 / 10_000_000.0;
        let messages: std::collections::BTreeMap<_, _> = MESSAGES.with(|messages| {
            messages
                .borrow()
                .iter()
                .filter_map(|(key, count)| {
                    let delta = count - messages_before.get(key).copied().unwrap_or(0);
                    (delta > 0).then(|| {
                        (
                            format!(
                                "{}:0x{:04x}",
                                if key.0 == 0 { "main" } else { "queue" },
                                key.1
                            ),
                            delta,
                        )
                    })
                })
                .collect()
        });
        let delta: Vec<_> = counters()
            .into_iter()
            .zip(before)
            .map(|(a, b)| a - b)
            .collect();
        Ok(
            serde_json::json!({"state":state,"seconds":seconds,"cpu_seconds":cpu_seconds,
        "cpu_one_core_percent":100.0*cpu_seconds/seconds,"ui_thread_cpu_seconds":ui_seconds,"ui_thread_one_core_percent":100.0*ui_seconds/seconds,"message_waits":WAITS.load(Ordering::Relaxed)-waits_before,"messages":messages,
        "cpu_machine_percent":100.0*cpu_seconds/seconds/std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1) as f64,
        "main_rasters":delta[0],"group_rasters":delta[1],"layouts":delta[2],"owner_draws":delta[3],"wm_paint":delta[4],"wm_printclient":delta[5],"refreshes":delta[6],"status_updates":delta[7],"main_blits":delta[8]}),
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
        let mut settings_check = serde_json::json!({"http_warning_shown":http_note,"https_warning_hidden":https_note_hidden,"http_url_and_startup_opt_out_persisted":settings_persisted,"synthetic_token_dpapi_roundtrip":settings_persisted,"token_field_cleared":token_field_cleared,"live_provider_requests":0});
        let mut preferences_verified = true;
        for (language, theme) in [
            (Language::Russian, Theme::Dark),
            (Language::French, Theme::Light),
            (Language::German, Theme::Dark),
            (Language::Spanish, Theme::Light),
            (Language::Italian, Theme::Dark),
            (Language::English, Theme::Light),
        ] {
            let (language_control, theme_control) = APP.with(|cell| {
                let borrow = cell.borrow();
                let app = borrow.as_ref().unwrap();
                (app.c(LANGUAGE), app.c(THEME))
            });
            SendMessageW(language_control, CB_SETCURSEL, language as usize, 0);
            SendMessageW(theme_control, CB_SETCURSEL, theme as usize, 0);
            SendMessageW(
                hwnd,
                WM_COMMAND,
                LANGUAGE as usize | ((CBN_SELCHANGE as usize) << 16),
                0,
            );
            preferences_verified &= APP.with(|cell| {
                let borrow = cell.borrow();
                let app = borrow.as_ref().unwrap();
                app.layout.settings.language == language
                    && app.layout.settings.theme == theme
                    && app.text(PAGE_SETTINGS) == i18n::translate(language, "Settings")
                    && i18n::dark() == (theme == Theme::Dark)
            });
        }
        ensure!(
            preferences_verified,
            "Native language/theme switching regression"
        );
        settings_check["six_languages_and_both_themes_switch_without_restart"] =
            serde_json::json!(preferences_verified);
        APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            app.layout.items.clear();
            for group in &mut app.layout.groups {
                group.items.clear();
            }
            let keep_group = app.layout.groups[0].id;
            app.layout
                .import(app.data.join("Previous folder/Keep.txt"), false, keep_group);
            app.source_ids = None;
            app.scope.clear();
            app.set(JSON, "");
            app.organize_after_scan = true;
            app.refresh();
        });
        SendMessageW(hwnd, WM_COMMAND, PAGE_AI as usize, 0);
        let empty_onboarding = APP.with(|cell| {
            let borrow = cell.borrow();
            let app = borrow.as_ref().unwrap();
            GetWindowLongPtrW(app.c(ADD_DESKTOP), GWL_STYLE) as u32 & WS_VISIBLE != 0
                && GetWindowLongPtrW(app.c(TIDY_ADD_FOLDER), GWL_STYLE) as u32 & WS_VISIBLE != 0
                && GetWindowLongPtrW(app.c(AI_ITEMS), GWL_STYLE) as u32 & WS_VISIBLE == 0
        });
        SendMessageW(hwnd, WM_COMMAND, SCAN as usize, 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            pump(Duration::from_millis(50))?;
            if APP.with(|cell| cell.borrow().as_ref().unwrap().task == 0) {
                break;
            }
            ensure!(Instant::now() < deadline, "Fixture import timed out");
        }
        let (imported, selected) = APP.with(|cell| {
            let borrow = cell.borrow();
            let app = borrow.as_ref().unwrap();
            (
                app.tidy_items.len(),
                SendMessageW(app.c(AI_ITEMS), LB_GETSELCOUNT, 0, 0) as usize,
            )
        });
        ensure!(
            empty_onboarding && imported > 0 && selected == imported,
            "Empty-to-import workflow regression"
        );
        SendMessageW(hwnd, WM_COMMAND, OFFLINE as usize, 0);
        SendMessageW(hwnd, WM_COMMAND, PREVIEW_MOVES as usize, 0);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            pump(Duration::from_millis(50))?;
            if APP.with(|cell| cell.borrow().as_ref().unwrap().task == 0) {
                break;
            }
            ensure!(Instant::now() < deadline, "Move preview timed out");
        }
        let file_review = APP.with(|cell| {
            let borrow = cell.borrow();
            let app = borrow.as_ref().unwrap();
            app.page == 5
                && app
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.moves.len() == imported)
                && GetWindowLongPtrW(app.c(MOVES), GWL_STYLE) as u32 & WS_VISIBLE != 0
        });
        ensure!(file_review, "Desktop folder review regression");
        let preference_preserves_review = APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            let preview = app.plan_json.clone();
            SendMessageW(app.c(THEME), CB_SETCURSEL, 1, 0);
            app.change_preferences();
            let preserved =
                app.plan.is_some() && app.plan_json == preview && app.text(JSON) == preview;
            SendMessageW(app.c(THEME), CB_SETCURSEL, 0, 0);
            app.change_preferences();
            preserved && app.plan.is_some() && app.plan_json == preview && app.text(JSON) == preview
        });
        ensure!(
            preference_preserves_review,
            "Theme switch discarded a reviewed move plan"
        );
        settings_check["theme_switch_preserves_reviewed_moves"] =
            serde_json::json!(preference_preserves_review);

        SendMessageW(hwnd, WM_COMMAND, BACK_TO_PLAN as usize, 0);
        SendMessageW(hwnd, WM_COMMAND, APPLY_VISUAL as usize, 0);
        let offline_groups = APP.with(|cell| -> Result<usize> {
            let borrow = cell.borrow();
            let app = borrow.as_ref().unwrap();
            app.layout.validate()?;
            Ok(app.layout.groups.len())
        })?;
        let connection_states = APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            let revision = app.ai_revision;
            app.ai_test_result(revision, Ok(()));
            let connected = app.ai_connection == AiConnection::Connected
                && GetWindowLongPtrW(app.c(AI_CONNECTION), GWLP_USERDATA) == 1;
            app.ai_edited();
            app.ai_test_result(revision, Ok(()));
            let stale_ignored = app.ai_connection == AiConnection::NotTested;
            app.ai_test_result(
                app.ai_revision,
                Err(anyhow::anyhow!("Synthetic connection failure")),
            );
            let failed = app.ai_connection == AiConnection::Failed;
            app.set_ai_connection(AiConnection::NotTested, "AI not tested");
            connected && stale_ignored && failed
        });
        ensure!(connection_states, "AI connection indication regression");
        let mut group_toggle = true;
        APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            app.hidden = true;
            app.show_groups();
        });
        for visible in [true, false, true, false] {
            SendMessageW(hwnd, WM_COMMAND, SHOW_GROUPS as usize, 0);
            group_toggle &= APP.with(|cell| {
                let borrow = cell.borrow();
                let app = borrow.as_ref().unwrap();
                app.windows
                    .values()
                    .all(|window| (IsWindowVisible(window.hwnd) != 0) == visible)
                    && app.text(SHOW_GROUPS)
                        == if visible {
                            "Hide groups"
                        } else {
                            "Show groups"
                        }
            });
        }
        ensure!(group_toggle, "Show/hide groups toggle regression");
        let workflow_check = serde_json::json!({"empty_onboarding":empty_onboarding,"fixture_items_imported":imported,"first_import_selected":selected,"file_review_opened_without_moving":file_review,"offline_groups_applied":offline_groups,"group_toggle_four_transitions":group_toggle,"connection_success_failure_and_stale_ui_states":connection_states,"connection_ui_scope":"synthetic completion events; actual probe transport is tested separately against loopback"});
        SendMessageW(hwnd, WM_COMMAND, PAGE_LIBRARY as usize, 0);
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
        let before_exposure = MAIN_PAINTS.load(Ordering::Relaxed);
        for _ in 0..10 {
            InvalidateRect(hwnd, null(), 0);
            UpdateWindow(hwnd);
        }
        let exposure_rasters = MAIN_PAINTS.load(Ordering::Relaxed) - before_exposure;
        ensure!(
            exposure_rasters == 0,
            "Unchanged main exposures rebuilt the scene"
        );
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
        let restored_followup = sample("restored follow-up")?;
        let tray_removed = APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let app = borrow.as_mut().unwrap();
            if !app.tray_available {
                return true;
            }
            let removed = app.tray(false).is_ok();
            if removed {
                app.tray_available = false;
            }
            removed
        });
        let report = serde_json::json!({"pid":std::process::id(),"executable":std::env::current_exe()?,"profile":"release",
        "backend":"CPU GDI / native Win32 controls; no GPU renderer","fixture_items":APP.with(|c|c.borrow().as_ref().unwrap().layout.items.len()),
        "tray_registered":tray_registered,"tray_removed_before_exit":tray_removed,"system_close_preserved_window":close_preserved_window,"system_close_hid_to_tray":close_hid_to_tray,
        "restore_visible":restored_visible,"settings":settings_check,"workflow":workflow_check,"unchanged_exposures":10,"unchanged_exposure_rasters":exposure_rasters,"navigation":transitions,"samples":[visible,after_navigation,hidden,restored,restored_followup],
        "gpu_telemetry":"not collected; compositor/driver timing is not certified",
        "scope":"isolated in-process integration and idle load; physical tray clicks and keyboard traversal not tested"});
        fs::create_dir_all(options.output.parent().context("Missing report parent")?)?;
        fs::write(&options.output, serde_json::to_vec_pretty(&report)?)?;
        ensure!(
            close_preserved_window && restored_visible && tray_removed,
            "Native close/restore regression"
        );
        ensure!(
            wrong_visibility == 0 && FONT_CREATIONS.load(Ordering::Relaxed) == fonts_before,
            "Navigation visibility or font recreation regression"
        );
        Ok(())
    }
}
