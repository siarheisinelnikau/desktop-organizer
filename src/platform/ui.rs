//! Native UI. All App state belongs to the message-loop thread. Reentrant messages
//! use DefWindowProc instead of borrowing the App twice; workers publish messages only.
#![allow(unsafe_op_in_unsafe_fn)]
use crate::{
    ai, grouping,
    model::{Item, Layout, MAX_ITEMS, Placement},
    operations::{MoveState, Plan},
    persistence, updates,
    worker::{Event, Job, Worker},
};
use anyhow::{Context, Result, ensure};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::OsStr,
    fs,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{Com::*, LibraryLoader::*, Threading::*},
        UI::{
            Accessibility::*,
            Controls::{Dialogs::*, *},
            HiDpi::*,
            Input::KeyboardAndMouse::*,
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
    core::*,
};
mod capture;
mod diagnostic;
mod theme;
const PROPOSALS: u16 = 180;
const PROPOSAL_NAME: u16 = 181;
const RENAME_PROPOSAL: u16 = 182;
const ASSIGN_PROPOSAL: u16 = 183;
const AI_SETUP: u16 = 184;
const FOLDER_NAME: u16 = 185;
const OPACITY_SLIDER: u16 = 186;
const OPACITY_VALUE: u16 = 187;
// TBM_GETPOS is WM_USER + 0; the pinned metadata omits this alias.
const TBM_GETPOS: u32 = WM_USER;

const MAIN_CLASS: PCWSTR = w!("DesktopOrganizer.Settings.v1");
const GROUP_CLASS: PCWSTR = w!("DesktopOrganizer.Group.v1");
const WORK: u32 = WM_APP + 1;
const RESTORE: u32 = WM_APP + 2;
const TRAY: u32 = WM_APP + 3;
const FOREGROUND: u32 = WM_APP + 4;
const START: u32 = WM_APP + 5;
const SAVE_TIMER: usize = 1;
const PAGE_LIBRARY: u16 = 101;
const PAGE_AI: u16 = 102;
const PAGE_SETTINGS: u16 = 103;
const SHOW_GROUPS: u16 = 104;
const ROOT: u16 = 110;
const BROWSE: u16 = 111;
const SCAN: u16 = 112;
const GROUPS: u16 = 120;
const ITEMS: u16 = 121;
const NEW_GROUP: u16 = 122;
const DELETE_GROUP: u16 = 123;
const GROUP_NAME: u16 = 124;
const RENAME_GROUP: u16 = 125;
const REMOVE: u16 = 126;
const ASSIGN: u16 = 127;
const AI_ITEMS: u16 = 130;
const SELECT_ALL: u16 = 131;
const OFFLINE: u16 = 132;
const CONSENT: u16 = 133;
const REQUEST_AI: u16 = 134;
const JSON: u16 = 135;
const APPLY_VISUAL: u16 = 136;
const MOVE_OPT: u16 = 137;
const PREVIEW_MOVES: u16 = 138;
const MOVES: u16 = 139;
const APPLY_MOVES: u16 = 140;
const UNDO: u16 = 141;
const BASE: u16 = 150;
const MODEL: u16 = 151;
const KEY: u16 = 152;
const CLEAR_KEY: u16 = 153;
const AUTO_CHECK: u16 = 154;
const FULLSCREEN: u16 = 155;
const OPACITY: u16 = 156;
const ACCENT: u16 = 157;
const SAVE_SETTINGS: u16 = 158;
const CHECK: u16 = 159;
const INSTALL: u16 = 160;
const UPDATE_STATUS: u16 = 161;
const STATUS: u16 = 170;
const QUIT: u16 = 171;
const TRAY_OPEN: u16 = 201;
const TRAY_SHOW: u16 = 202;
const TRAY_HIDE: u16 = 203;
const TRAY_QUIT: u16 = 204;

static HOST: AtomicUsize = AtomicUsize::new(0);
static FOREGROUND_PENDING: AtomicBool = AtomicBool::new(false);
thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }

struct Control {
    hwnd: HWND,
    id: u16,
    page: u8,
    rect: (i32, i32, i32, i32),
    placed: Option<(i32, i32, i32, i32)>,
}
struct GroupWindow {
    hwnd: HWND,
    scroll: i32,
    selected: Option<u64>,
}
struct Drag {
    item: u64,
    origin: POINT,
    moved: bool,
}
struct App {
    hwnd: HWND,
    layout: Layout,
    data: PathBuf,
    worker: Worker,
    controls: Vec<Control>,
    windows: HashMap<u64, GroupWindow>,
    page: u8,
    font: HFONT,
    font_dpi: u32,
    icon: HICON,
    small_icon: HICON,
    task: u8,
    scope: Vec<Item>,
    plan: Option<Plan>,
    plan_json: String,
    available: Option<updates::Available>,
    drag: Option<Drag>,
    hidden: bool,
    fullscreen: bool,
    save_sequence: u64,
    quitting: Option<u64>,
    pending_install: Option<updates::Prepared>,
    dirty: bool,
    main_scroll: POINT,
    tray_message: u32,
    tray_available: bool,
    hooks: Vec<HWINEVENTHOOK>,
    ai_setup_open: bool,
}
static CAPTURE_DPI: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static MAIN_PAINTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GROUP_PAINTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LAYOUTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static OWNER_DRAWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static FONT_CREATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PAGE_SWITCHES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn window_dpi(hwnd: HWND) -> u32 {
    let override_dpi = CAPTURE_DPI.load(Ordering::Relaxed);
    if override_dpi > 0 {
        override_dpi
    } else {
        unsafe { GetDpiForWindow(hwnd).max(96) }
    }
}

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
fn word(v: usize) -> u16 {
    (v & 0xffff) as u16
}
fn signed_low(v: isize) -> i32 {
    (v as u16 as i16) as i32
}
fn signed_high(v: isize) -> i32 {
    ((v >> 16) as u16 as i16) as i32
}
fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64) / 96) as i32
}
fn unscale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * 96) / dpi.max(1) as i64) as i32
}

pub fn run() {
    let mut arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|arg| arg == "--cleanup-update")
        && arguments.len() == 2
    {
        arguments.clear();
    }
    let diagnostic = match capture::Options::parse(arguments.into_iter()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(2);
        }
    };
    unsafe {
        if let Err(error) = startup(diagnostic) {
            if std::env::args_os().any(|a| a == "--capture" || a == "--diagnose") {
                eprintln!("{error:#}");
                std::process::exit(2);
            }
            MessageBoxW(
                null_mut(),
                wide(format!("{error:#}")).as_ptr(),
                w!("DesktopOrganizer could not start"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
unsafe fn startup(diagnostic: Option<capture::Options>) -> Result<()> {
    trace("startup");
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let data = if let Some(options) = &diagnostic {
        options
            .output
            .parent()
            .context("Capture output needs a parent directory")?
            .to_path_buf()
    } else {
        persistence::default_data_dir()?
    };
    let mutex_name = if diagnostic.is_some() {
        wide(format!(
            "Local\\DesktopOrganizer.Capture.{}",
            std::process::id()
        ))
    } else {
        wide("Local\\DesktopOrganizer.Interactive.v1")
    };
    let mutex = CreateMutexW(null(), 0, mutex_name.as_ptr());
    ensure!(!mutex.is_null(), "Cannot create instance lock");
    if GetLastError() == ERROR_ALREADY_EXISTS {
        let previous = FindWindowW(MAIN_CLASS, null());
        if !previous.is_null() {
            let mut pid = 0;
            GetWindowThreadProcessId(previous, &mut pid);
            AllowSetForegroundWindow(pid);
            PostMessageW(previous, RESTORE, 0, 0);
        }
        CloseHandle(mutex);
        return Ok(());
    }
    // A corrupt or future schema is preserved; never silently replace it with defaults.
    let state = data.join("layout.json");
    let layout: Layout = if let Some(options) = &diagnostic {
        options.layout()?
    } else if state.exists() {
        persistence::load(&state)?
    } else {
        Layout::default()
    };
    layout.validate()?;
    let instance = GetModuleHandleW(null());
    let icon = theme::application_icon(instance, false)?;
    let small_icon = theme::application_icon(instance, true)?;
    for (class, proc) in [
        (
            MAIN_CLASS,
            Some(main_proc as unsafe extern "system" fn(_, _, _, _) -> _),
        ),
        (
            GROUP_CLASS,
            Some(group_proc as unsafe extern "system" fn(_, _, _, _) -> _),
        ),
    ] {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as _,
            style: CS_DBLCLKS,
            lpfnWndProc: proc,
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hIcon: icon,
            hIconSm: small_icon,
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszClassName: class,
            ..zeroed()
        };
        ensure!(
            RegisterClassExW(&wc) != 0,
            "Could not register native window class"
        );
    }
    CoInitializeEx(null(), COINIT_APARTMENTTHREADED as _);
    let common = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as _,
        dwICC: ICC_BAR_CLASSES,
    };
    InitCommonControlsEx(&common);
    let hwnd = CreateWindowExW(
        WS_EX_COMPOSITED,
        MAIN_CLASS,
        w!("DesktopOrganizer — Library and settings"),
        WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_HSCROLL | WS_VSCROLL,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        1050,
        800,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    ensure!(!hwnd.is_null(), "Could not create settings window");
    let app = App {
        hwnd,
        layout,
        data,
        worker: Worker::start(hwnd as usize, WORK),
        controls: vec![],
        windows: HashMap::new(),
        page: 1,
        font: null_mut(),
        font_dpi: 0,
        icon,
        small_icon,
        task: 0,
        scope: vec![],
        plan: None,
        plan_json: String::new(),
        available: None,
        drag: None,
        hidden: diagnostic.is_some(),
        fullscreen: false,
        save_sequence: 0,
        quitting: None,
        pending_install: None,
        dirty: false,
        main_scroll: POINT::default(),
        tray_message: RegisterWindowMessageW(w!("TaskbarCreated")),
        tray_available: false,
        hooks: vec![],
        ai_setup_open: false,
    };
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
    if let Some(options) = &diagnostic {
        CAPTURE_DPI.store(options.dpi, Ordering::Relaxed);
    }
    APP.with(|cell| -> Result<()> {
        let mut borrow = cell.borrow_mut();
        let app = borrow.as_mut().unwrap();
        app.create_controls();
        app.refresh();
        app.rebuild_groups()?;
        if let Some(options) = &diagnostic {
            app.page = options.page();
            app.ai_setup_open = options.scenario == "ai-setup";
            if app.ai_setup_open {
                app.set(AI_SETUP, "Done");
            }
            app.position_controls();
            app.scope = app.layout.items.clone();
            if !app.scope.is_empty() {
                app.set_suggestions(grouping::offline(&app.scope))?;
            }
            app.status("Files stay in place.");
            if options.scenario == "error" {
                app.status(
                    "Folder unavailable. Choose another folder to keep organizing your files.",
                );
            }
            return Ok(());
        }
        app.hooks.push(SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(foreground_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        ));
        app.hooks.push(SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            null_mut(),
            Some(foreground_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        ));
        app.position_controls();
        Ok(())
    })?;
    if diagnostic.is_none() {
        super::updater::cleanup_after_startup();
    }
    if let Some(options) = diagnostic {
        let result = if options.diagnose {
            diagnostic::run(&options, hwnd)
        } else {
            capture::save(&options, hwnd)
        };
        APP.with(|cell| {
            if let Some(app) = cell.borrow_mut().take() {
                for window in app.windows.values() {
                    DestroyWindow(window.hwnd);
                }
                if !app.font.is_null() {
                    DeleteObject(app.font);
                }
                DestroyIcon(app.icon);
                DestroyIcon(app.small_icon);
            }
        });
        DestroyWindow(hwnd);
        CoUninitialize();
        CloseHandle(mutex);
        return result;
    }
    HOST.store(hwnd as usize, Ordering::Release);
    let monitor = monitor_info(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST));
    let area = monitor.monitorInfo.rcWork;
    let dpi = window_dpi(hwnd).max(96);
    SetWindowPos(
        hwnd,
        null_mut(),
        area.left,
        area.top,
        scale(1050, dpi).min(area.right - area.left),
        scale(800, dpi).min(area.bottom - area.top),
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    ShowWindow(hwnd, SW_SHOW);
    trace("settings shown");
    PostMessageW(hwnd, START, 0, 0);
    let mut message: MSG = zeroed();
    loop {
        let result = GetMessageW(&mut message, null_mut(), 0, 0);
        if result <= 0 {
            trace(&format!("message loop ended: {result}"));
            break;
        }
        if IsDialogMessageW(hwnd, &message) == 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    HOST.store(0, Ordering::Release);
    APP.with(|cell| {
        if let Some(mut app) = cell.borrow_mut().take() {
            for hook in app.hooks.drain(..) {
                if !hook.is_null() {
                    UnhookWinEvent(hook);
                }
            }
            let _ = app.tray(false);
            if !app.font.is_null() {
                DeleteObject(app.font);
            }
            for window in app.windows.values() {
                DestroyWindow(window.hwnd);
            }
            DestroyIcon(app.icon);
            DestroyIcon(app.small_icon);
        }
    });
    CoUninitialize();
    CloseHandle(mutex);
    Ok(())
}
fn trace(message: &str) {
    use std::io::Write;
    if std::env::var_os("DESKTOP_ORGANIZER_TRACE").is_some()
        && let Ok(data) = persistence::default_data_dir()
        && let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(data.join("ui-trace.log"))
    {
        let _ = writeln!(file, "{message}");
    }
}

unsafe extern "system" fn foreground_event(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if event == EVENT_OBJECT_LOCATIONCHANGE
        && (object != OBJID_WINDOW || hwnd != GetForegroundWindow())
    {
        return;
    }
    let host = HOST.load(Ordering::Acquire);
    if host != 0 && !FOREGROUND_PENDING.swap(true, Ordering::AcqRel) {
        PostMessageW(host as _, FOREGROUND, 0, 0);
    }
}
unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_DRAWITEM {
        return theme::draw_control(lp as *const DRAWITEMSTRUCT);
    }
    if msg == WM_MEASUREITEM {
        let item = &mut *(lp as *mut MEASUREITEMSTRUCT);
        if item.CtlType == ODT_LISTBOX {
            item.itemHeight = scale(48, window_dpi(hwnd)) as _;
            return 1;
        }
    }
    if msg == WM_CTLCOLORSTATIC {
        SetBkMode(wp as HDC, TRANSPARENT as _);
        SetTextColor(wp as HDC, theme::rgb(43, 54, 69));
        SetBkColor(wp as HDC, theme::rgb(255, 255, 255));
        return GetStockObject(WHITE_BRUSH) as LRESULT;
    }
    // DestroyWindow sends this synchronously while App is already borrowed.
    // Quit must work without a paint or a second mutable borrow, including when hidden.
    if msg == WM_DESTROY {
        trace("settings destroyed");
        PostQuitMessage(0);
        return 0;
    }
    let handled=APP.with(|cell| {
        let Ok(mut borrow) = cell.try_borrow_mut() else {
            if msg == FOREGROUND { FOREGROUND_PENDING.store(false, Ordering::Release); }
            return None;
        };
        let app = borrow.as_mut()?;
        let result = match msg {
            WM_COMMAND => {
                let id = word(wp); let notification = (wp >> 16) as u16;
                if id == GROUPS && notification == LBN_SELCHANGE as u16 { app.group_selection(); return Some(0); }
                if id == PROPOSALS && notification == LBN_SELCHANGE as u16 { app.proposal_selection(); return Some(0); }
                if ((id==ITEMS||id==AI_ITEMS)&&notification==LBN_SELCHANGE as u16)||(id==ROOT&&notification==EN_CHANGE as u16) {app.update_actions();return Some(0);}
                if notification == BN_CLICKED as u16 { if let Err(e) = app.command(id) { app.error(e); } app.update_actions(); }
                0
            },
            WM_SIZE => { app.position_controls(); 0 },
            WM_PAINT => { app.paint_main(); 0 },
            WM_PRINTCLIENT => { app.render_main(wp as HDC); 0 },
            WM_ERASEBKGND => 1,
            WM_GETMINMAXINFO => { let info = &mut *(lp as *mut MINMAXINFO); info.ptMinTrackSize = POINT { x: scale(640, window_dpi(hwnd)), y: scale(420, window_dpi(hwnd)) }; 0 },
            WM_HSCROLL if lp==app.c(OPACITY_SLIDER)as isize => {let value=SendMessageW(app.c(OPACITY_SLIDER),TBM_GETPOS,0,0);app.set(OPACITY_VALUE,&format!("{value}%"));0},
            WM_VSCROLL | WM_HSCROLL => { app.scroll_main(msg,wp); 0 },
            WM_MOUSEWHEEL => { let delta=((wp>>16) as u16 as i16) as i32; app.main_scroll.y=(app.main_scroll.y-scale(delta,window_dpi(hwnd))).max(0); app.position_controls(); 0 },
            WM_DPICHANGED => { let r = &*(lp as *const RECT); SetWindowPos(hwnd, null_mut(), r.left, r.top, r.right-r.left, r.bottom-r.top, SWP_NOZORDER | SWP_NOACTIVATE); app.position_controls(); 0 },
            WM_CLOSE => { if app.tray_available { ShowWindow(hwnd, SW_HIDE); } else { app.status("Tray is unavailable. Use Save and quit to exit; settings remain accessible."); } 0 },
            RESTORE => { app.restore(); 0 },
            START => {
                app.status("Add a folder to get started. Your files stay in place.");
                app.register_tray();
                if app.layout.settings.startup_update_check && let Err(e) = app.worker.submit(Job::Check) { app.set_update(&format!("Startup update check unavailable: {e}")); }
                let journal=app.data.join("operation.json");
                if journal.exists() && let Err(e)=app.worker.submit(Job::Recover(journal)) { app.error(e); }
                let failure = app.data.join("update-error.txt");
                if let Ok(message) = fs::read_to_string(&failure) { app.set_update(&format!("Previous update failed: {message}")); let _ = fs::remove_file(failure); }
                0
            },
            WORK => { app.worker_events(); 0 },
            WM_TIMER if wp == SAVE_TIMER => { KillTimer(hwnd, SAVE_TIMER); if let Err(e) = app.flush_save() { app.error(e); } 0 },
            WM_DROPFILES => { app.drop_files(wp as _, app.selected_group()); 0 },
            FOREGROUND => { FOREGROUND_PENDING.store(false, Ordering::Release); app.fullscreen_policy(); 0 },
            TRAY => {
                match word(lp as usize) as u32 {
                    WM_LBUTTONUP | NIN_SELECT | 0x401 => app.restore(),
                    WM_RBUTTONUP | WM_CONTEXTMENU => app.tray_menu(), _ => {},
                } 0
            },
            WM_QUERYENDSESSION => { app.status("Session ending. Recent changes are saved by the background writer."); 1 },
            WM_DESTROY => { PostQuitMessage(0); 0 },
            _ if msg == app.tray_message => { app.register_tray(); 0 },
            _ => return None,
        };
        // Native menus and dialogs run nested message loops. Their wake message can
        // arrive while App is borrowed, so always drain results after returning.
        if msg != WORK { app.worker_events(); }
        Some(result)
    });
    // DefWindowProc can synchronously send WM_CLOSE, WM_SIZE and other messages.
    // Release App before calling it so those messages reach our actual handlers.
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp))
}

unsafe extern "system" fn group_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        let hit = DefWindowProcW(hwnd, msg, wp, lp);
        if hit == HTCLIENT as isize {
            let mut point = POINT {
                x: signed_low(lp),
                y: signed_high(lp),
            };
            ScreenToClient(hwnd, &mut point);
            if point.y >= 0 && point.y < scale(70, window_dpi(hwnd)) {
                return HTCAPTION as isize;
            }
        }
        return hit;
    }
    let handled = APP.with(|cell| {
        let Ok(mut borrow) = cell.try_borrow_mut() else {
            return None;
        };
        let app = borrow.as_mut()?;
        let id = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u64;
        if id == 0 {
            return None;
        }
        let result = match msg {
            WM_PAINT => {
                if IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
                    app.paint_group(id);
                } else {
                    let mut ps: PAINTSTRUCT = zeroed();
                    BeginPaint(hwnd, &mut ps);
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_PRINTCLIENT => {
                app.render_group(id, wp as HDC);
                0
            }
            WM_ERASEBKGND => 1,
            WM_EXITSIZEMOVE => {
                app.capture_placement(id);
                app.changed();
                0
            }
            WM_DPICHANGED => {
                let r = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                app.capture_placement(id);
                app.changed();
                InvalidateRect(hwnd, null(), 0);
                0
            }
            WM_DISPLAYCHANGE => {
                app.recover_positions();
                0
            }
            WM_GETMINMAXINFO => {
                let p = &mut *(lp as *mut MINMAXINFO);
                let dpi = window_dpi(hwnd);
                p.ptMinTrackSize = POINT {
                    x: scale(240, dpi),
                    y: scale(90, dpi),
                };
                0
            }
            WM_CLOSE | WM_NCLBUTTONDBLCLK => {
                if msg == WM_CLOSE || wp == HTCAPTION as usize {
                    app.collapse(id);
                    return Some(0);
                }
                return None;
            }
            WM_DROPFILES => {
                app.drop_files(wp as _, id);
                0
            }
            WM_MOUSEWHEEL => {
                let delta = ((wp >> 16) as u16 as i16) as i32;
                if let Some(window) = app.windows.get_mut(&id) {
                    window.scroll = (window.scroll - delta / 120 * 90).max(0);
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_LBUTTONDOWN => {
                if let Some(item) = app.hit_item(id, lp) {
                    if let Some(window) = app.windows.get_mut(&id) {
                        window.selected = Some(item);
                    }
                    let mut origin = POINT {
                        x: signed_low(lp),
                        y: signed_high(lp),
                    };
                    ClientToScreen(hwnd, &mut origin);
                    app.drag = Some(Drag {
                        item,
                        origin,
                        moved: false,
                    });
                    SetCapture(hwnd);
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_MOUSEMOVE => {
                if let Some(drag) = &mut app.drag {
                    let mut point = POINT::default();
                    GetCursorPos(&mut point);
                    if (point.x - drag.origin.x).abs() + (point.y - drag.origin.y).abs() > 8 {
                        drag.moved = true;
                        SetCursor(LoadCursorW(null_mut(), IDC_SIZEALL));
                    }
                }
                0
            }
            WM_LBUTTONUP => {
                let drag = app.drag.take();
                ReleaseCapture();
                if let Some(drag) = drag
                    && drag.moved
                    && app.task == 0
                {
                    let mut point = POINT::default();
                    GetCursorPos(&mut point);
                    let target = GetAncestor(WindowFromPoint(point), GA_ROOT);
                    let target_id = app
                        .windows
                        .iter()
                        .find_map(|(id, w)| (w.hwnd == target).then_some(*id));
                    if let Some(target_id) = target_id {
                        app.layout.assign(drag.item, target_id);
                        app.refresh();
                        app.invalidate_groups();
                        app.changed();
                    }
                }
                0
            }
            WM_CAPTURECHANGED => {
                app.drag = None;
                0
            }
            WM_LBUTTONDBLCLK => {
                if let Some(item) = app.hit_item(id, lp) {
                    app.open_item(item);
                }
                0
            }
            WM_KEYDOWN if wp == VK_RETURN as usize => {
                if let Some(item) = app.windows.get(&id).and_then(|w| w.selected) {
                    app.open_item(item);
                }
                0
            }
            WM_KEYDOWN if wp == VK_DELETE as usize => {
                if app.task == 0
                    && let Some(item) = app.windows.get(&id).and_then(|w| w.selected)
                {
                    app.remove_items(&[item]);
                }
                0
            }
            WM_CONTEXTMENU | WM_NCRBUTTONUP => {
                app.group_menu(id);
                0
            }
            _ => return None,
        };
        app.worker_events();
        Some(result)
    });
    handled.unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp))
}

impl App {
    unsafe fn control(
        &mut self,
        id: u16,
        page: u8,
        class: PCWSTR,
        text: &str,
        style: u32,
        rect: (i32, i32, i32, i32),
    ) {
        let is_list = matches!(id, GROUPS | ITEMS | AI_ITEMS | PROPOSALS);
        let style = if is_list {
            (style & !WS_HSCROLL) | LBS_OWNERDRAWFIXED as u32 | LBS_HASSTRINGS as u32
        } else {
            style
        };
        let is_edit = matches!(
            id,
            ROOT | GROUP_NAME | JSON | MOVES | BASE | MODEL | KEY | OPACITY
        );
        let hwnd = CreateWindowExW(
            if is_edit { WS_EX_CLIENTEDGE } else { 0 },
            class,
            wide(text).as_ptr(),
            WS_CHILD | if id == 0 { 0 } else { WS_TABSTOP } | style,
            0,
            0,
            0,
            0,
            self.hwnd,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        self.controls.push(Control {
            hwnd,
            id,
            page,
            rect,
            placed: None,
        });
        if is_edit {
            SendMessageW(
                hwnd,
                EM_SETLIMITTEXT,
                if id == JSON || id == MOVES {
                    128 * 1024
                } else {
                    4096
                },
                0,
            );
        }
    }
    unsafe fn label(&mut self, page: u8, text: &str, rect: (i32, i32, i32, i32)) {
        self.control(0, page, w!("STATIC"), text, 0, rect);
    }
    unsafe fn button(&mut self, id: u16, page: u8, text: &str, rect: (i32, i32, i32, i32)) {
        self.control(id, page, w!("BUTTON"), text, BS_PUSHBUTTON as _, rect);
    }
    unsafe fn edit(
        &mut self,
        id: u16,
        page: u8,
        text: &str,
        rect: (i32, i32, i32, i32),
        extra: u32,
    ) {
        self.control(
            id,
            page,
            w!("EDIT"),
            text,
            ES_AUTOHSCROLL as u32 | extra,
            rect,
        );
    }
    unsafe fn check(
        &mut self,
        id: u16,
        page: u8,
        text: &str,
        rect: (i32, i32, i32, i32),
        checked: bool,
    ) {
        self.control(id, page, w!("BUTTON"), text, BS_AUTOCHECKBOX as _, rect);
        SendMessageW(
            self.c(id),
            BM_SETCHECK,
            if checked { BST_CHECKED } else { BST_UNCHECKED } as _,
            0,
        );
    }
    unsafe fn create_controls(&mut self) {
        self.button(PAGE_LIBRARY, 0, "Library", (20, 18, 160, 34));
        self.button(PAGE_AI, 0, "Organize / AI", (190, 18, 180, 34));
        self.button(PAGE_SETTINGS, 0, "Settings / Updates", (380, 18, 200, 34));
        self.button(SHOW_GROUPS, 0, "Show groups", (790, 18, 180, 34));
        self.label(1, "Source folder", (20, 78, 105, 25));
        let root = self.layout.settings.root.to_string_lossy().into_owned();
        self.edit(ROOT, 1, &root, (130, 72, 615, 30), 0);
        self.button(BROWSE, 1, "Browse...", (755, 72, 100, 30));
        self.button(SCAN, 1, "Import", (865, 72, 105, 30));
        self.label(1,"Import adds references only. Drag files onto any group; original Desktop icons remain unchanged.",(20,112,950,30));
        self.label(1, "GROUPS", (20, 150, 240, 24));
        self.control(
            GROUPS,
            1,
            w!("LISTBOX"),
            "",
            LBS_NOTIFY as u32 | WS_VSCROLL,
            (20, 180, 240, 280),
        );
        self.label(1, "ITEMS — Ctrl/Shift to select", (290, 150, 680, 24));
        self.control(
            ITEMS,
            1,
            w!("LISTBOX"),
            "",
            LBS_EXTENDEDSEL as u32 | LBS_NOTIFY as u32 | WS_VSCROLL | WS_HSCROLL,
            (290, 180, 680, 280),
        );
        self.edit(GROUP_NAME, 1, "Inbox", (20, 475, 240, 30), 0);
        self.button(RENAME_GROUP, 1, "Rename", (20, 515, 110, 32));
        self.button(NEW_GROUP, 1, "New group", (140, 515, 120, 32));
        self.button(DELETE_GROUP, 1, "Remove group", (20, 560, 240, 32));
        self.button(ASSIGN, 1, "Assign selected to group", (290, 475, 240, 32));
        self.button(REMOVE, 1, "Remove references", (545, 475, 200, 32));
        self.label(1,"All imported items appear in the list. Assign them to the group selected on the left.\r\nDouble-click a tile to open it. Drag tiles between groups. Use the mouse wheel to scroll.\r\nDouble-click a group title to collapse / expand. Right-click a group for its menu.\r\nRemoving a reference never deletes the original file.",(290,530,680,100));
        self.label(2,"Select the exact metadata to organize. AI sends name, extension, kind, category and local numeric ID only.",(20,78,950,25));
        self.control(
            AI_ITEMS,
            2,
            w!("LISTBOX"),
            "",
            LBS_EXTENDEDSEL as u32 | LBS_NOTIFY as u32 | WS_VSCROLL | WS_HSCROLL,
            (20, 110, 365, 280),
        );
        self.edit(
            JSON,
            2,
            "",
            (400, 110, 570, 280),
            ES_MULTILINE as u32
                | ES_WANTRETURN as u32
                | ES_AUTOVSCROLL as u32
                | WS_VSCROLL
                | WS_HSCROLL,
        );
        self.button(SELECT_ALL, 2, "Select all metadata", (20, 400, 180, 32));
        self.button(OFFLINE, 2, "Suggest offline", (210, 400, 175, 32));
        self.button(APPLY_VISUAL, 2, "Apply visual groups", (400, 400, 220, 32));
        self.check(
            CONSENT,
            2,
            "I allow sending the selected metadata to my configured AI endpoint.",
            (20, 443, 755, 28),
            false,
        );
        self.button(REQUEST_AI, 2, "Send selected to AI", (780, 440, 190, 32));
        self.check(
            MOVE_OPT,
            2,
            "Create folders and move files (optional; only direct files in the source folder)",
            (20, 480, 750, 28),
            false,
        );
        self.button(PREVIEW_MOVES, 2, "Preview moves", (20, 520, 180, 32));
        self.button(APPLY_MOVES, 2, "Apply previewed moves", (215, 520, 235, 32));
        self.button(UNDO, 2, "Undo last file operation", (470, 520, 240, 32));
        self.edit(MOVES,2,"No file operation planned. Edit names and item_ids in the JSON preview, then preview again.",(20,565,950,90),ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_HSCROLL);
        self.label(3, "APPEARANCE & BEHAVIOR", (20, 82, 440, 24));
        self.check(
            FULLSCREEN,
            3,
            "Hide groups while a foreground app covers its entire monitor",
            (20, 115, 650, 28),
            self.layout.settings.hide_on_fullscreen,
        );
        self.label(3, "Opacity (150-255)", (20, 160, 170, 25));
        self.edit(
            OPACITY,
            3,
            &self.layout.settings.opacity.to_string(),
            (200, 155, 90, 30),
            ES_NUMBER as _,
        );
        self.button(ACCENT, 3, "Choose accent...", (320, 155, 200, 30));
        self.label(3, "OPENAI-COMPATIBLE PROVIDER", (20, 210, 700, 24));
        self.label(3, "Base URL", (20, 250, 110, 25));
        self.edit(
            BASE,
            3,
            &self.layout.settings.ai_base_url.clone(),
            (140, 245, 830, 30),
            0,
        );
        self.label(3, "Model", (20, 290, 110, 25));
        self.edit(
            MODEL,
            3,
            &self.layout.settings.ai_model.clone(),
            (140, 285, 830, 30),
            0,
        );
        self.label(3, "New API token", (20, 330, 115, 25));
        self.edit(KEY, 3, "", (140, 325, 520, 30), ES_PASSWORD as _);
        self.check(
            CLEAR_KEY,
            3,
            "Remove saved token",
            (680, 325, 290, 30),
            false,
        );
        self.label(3,"Blank keeps the saved token. Windows DPAPI protects it for your user. ai-hub example: http://localhost:8080/v1 / universal.",(20,365,950,40));
        self.button(SAVE_SETTINGS, 3, "Save all settings", (20, 415, 200, 34));
        self.label(
            3,
            &format!(
                "UPDATES — Installed version {} — Windows x64 portable",
                updates::INSTALLED
            ),
            (20, 475, 950, 24),
        );
        self.check(
            AUTO_CHECK,
            3,
            "Check for updates on startup",
            (20, 512, 390, 28),
            self.layout.settings.startup_update_check,
        );
        self.button(CHECK, 3, "Check for updates", (430, 510, 240, 32));
        self.button(INSTALL, 3, "Download and install", (690, 510, 280, 32));
        self.label(3,"Canonical repository: github.com/siarheisinelnikau/desktop-organizer (publication pending)",(20,555,950,24));
        self.control(
            UPDATE_STATUS,
            3,
            w!("STATIC"),
            "No check completed in this session.",
            0,
            (20, 588, 950, 65),
        );
        self.control(STATUS, 0, w!("STATIC"), "Ready.", 0, (20, 685, 745, 50));
        self.button(QUIT, 0, "Save and quit", (790, 685, 180, 34));
        DragAcceptFiles(self.hwnd, 1);
        self.redesign_controls();
    }
    fn c(&self, id: u16) -> HWND {
        self.controls
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.hwnd)
            .unwrap_or(null_mut())
    }
    unsafe fn text(&self, id: u16) -> String {
        let hwnd = self.c(id);
        let n = GetWindowTextLengthW(hwnd).min(128 * 1024);
        let mut buf = vec![0u16; n as usize + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as _);
        String::from_utf16_lossy(&buf[..n as usize])
    }
    unsafe fn set(&self, id: u16, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\n', "\r\n");
        SetWindowTextW(self.c(id), wide(text).as_ptr());
        if id == ROOT {
            self.sync_folder_name();
        }
    }
    unsafe fn sync_folder_name(&self) {
        let path = PathBuf::from(self.text(ROOT));
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if path.as_os_str().is_empty() {
                    "Choose your first folder".into()
                } else {
                    path.to_string_lossy().into_owned()
                }
            });
        SetWindowTextW(self.c(FOLDER_NAME), wide(name).as_ptr());
    }
    unsafe fn checked(&self, id: u16) -> bool {
        SendMessageW(self.c(id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize
    }
    unsafe fn status(&self, text: &str) {
        self.set(STATUS, text);
    }
    unsafe fn error(&self, error: anyhow::Error) {
        self.status(&format!("Error: {error:#}"));
    }
    unsafe fn set_update(&self, text: &str) {
        self.set(UPDATE_STATUS, text);
    }
    unsafe fn position_controls(&mut self) {
        LAYOUTS.fetch_add(1, Ordering::Relaxed);
        let dpi = window_dpi(self.hwnd).max(96);
        let mut client = RECT::default();
        GetClientRect(self.hwnd, &mut client);
        if let Some(button) = self.controls.iter_mut().find(|c| c.id == AI_SETUP) {
            button.rect = if self.ai_setup_open {
                (772, 250, 188, 34)
            } else {
                (772, 326, 188, 38)
            };
        }
        for (bar, total, page, pos) in [
            (
                SB_HORZ,
                scale(1000, dpi),
                client.right,
                &mut self.main_scroll.x,
            ),
            (
                SB_VERT,
                scale(745, dpi),
                client.bottom,
                &mut self.main_scroll.y,
            ),
        ] {
            let mut info = SCROLLINFO {
                cbSize: size_of::<SCROLLINFO>() as _,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: total,
                nPage: page.max(0) as _,
                nPos: *pos,
                nTrackPos: 0,
            };
            SetScrollInfo(self.hwnd, bar, &info, 1);
            GetScrollInfo(self.hwnd, bar, &mut info);
            *pos = info.nPos;
        }
        let font_changed = self.font.is_null() || self.font_dpi != dpi;
        if font_changed {
            let font = CreateFontW(
                -scale(15, dpi),
                0,
                0,
                0,
                FW_NORMAL as _,
                0,
                0,
                0,
                DEFAULT_CHARSET as _,
                OUT_DEFAULT_PRECIS as _,
                CLIP_DEFAULT_PRECIS as _,
                CLEARTYPE_QUALITY as _,
                DEFAULT_PITCH as _,
                w!("Segoe UI"),
            );
            if !font.is_null() {
                FONT_CREATIONS.fetch_add(1, Ordering::Relaxed);
                for control in &self.controls {
                    SendMessageW(control.hwnd, WM_SETFONT, font as usize, 0);
                }
                if !self.font.is_null() {
                    DeleteObject(self.font);
                }
                self.font = font;
                self.font_dpi = dpi;
            }
        }
        let mut batch = BeginDeferWindowPos(self.controls.len() as i32);
        for control in &mut self.controls {
            let (x, y, w, h) = control.rect;
            let next = (
                scale(x, dpi) - self.main_scroll.x,
                scale(y, dpi) - self.main_scroll.y,
                scale(w, dpi),
                scale(h, dpi),
            );
            if control.placed != Some(next) {
                if !batch.is_null() {
                    batch = DeferWindowPos(
                        batch,
                        control.hwnd,
                        null_mut(),
                        next.0,
                        next.1,
                        next.2,
                        next.3,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
                    );
                } else {
                    SetWindowPos(
                        control.hwnd,
                        null_mut(),
                        next.0,
                        next.1,
                        next.2,
                        next.3,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
                    );
                }
                control.placed = Some(next);
            }
            if matches!(control.id, GROUPS | ITEMS | AI_ITEMS | PROPOSALS) {
                SendMessageW(control.hwnd, LB_SETITEMHEIGHT, 0, scale(48, dpi) as isize);
            }
        }
        if !batch.is_null() {
            EndDeferWindowPos(batch);
        }
        self.update_actions();
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_FRAME,
        );
    }
    unsafe fn switch_page(&mut self, page: u8) {
        if self.page == page {
            return;
        }
        let visible = IsWindowVisible(self.hwnd) != 0;
        if visible {
            SendMessageW(self.hwnd, WM_SETREDRAW, 0, 0);
        }
        self.page = page;
        PAGE_SWITCHES.fetch_add(1, Ordering::Relaxed);
        if page == 2 && SendMessageW(self.c(AI_ITEMS), LB_GETSELCOUNT, 0, 0) <= 0 {
            SendMessageW(self.c(AI_ITEMS), LB_SETSEL, 1, -1);
        }
        self.update_actions();
        if visible {
            SendMessageW(self.hwnd, WM_SETREDRAW, 1, 0);
        }
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_FRAME,
        );
    }
    unsafe fn scroll_main(&mut self, message: u32, wp: WPARAM) {
        let bar = if message == WM_VSCROLL {
            SB_VERT
        } else {
            SB_HORZ
        };
        let mut info: SCROLLINFO = zeroed();
        info.cbSize = size_of::<SCROLLINFO>() as _;
        info.fMask = SIF_ALL;
        GetScrollInfo(self.hwnd, bar, &mut info);
        let amount = scale(30, window_dpi(self.hwnd));
        let position = match word(wp) as i32 {
            SB_LINEUP => info.nPos - amount,
            SB_LINEDOWN => info.nPos + amount,
            SB_PAGEUP => info.nPos - info.nPage as i32,
            SB_PAGEDOWN => info.nPos + info.nPage as i32,
            SB_THUMBTRACK | SB_THUMBPOSITION => info.nTrackPos,
            SB_TOP => 0,
            SB_BOTTOM => info.nMax,
            _ => info.nPos,
        };
        if bar == SB_VERT {
            self.main_scroll.y = position.max(0);
        } else {
            self.main_scroll.x = position.max(0);
        }
        self.position_controls();
    }
    unsafe fn selected_group(&self) -> u64 {
        let row = SendMessageW(self.c(GROUPS), LB_GETCURSEL, 0, 0);
        self.layout
            .groups
            .get(row.max(0) as usize)
            .map(|g| g.id)
            .unwrap_or(1)
    }
    unsafe fn group_selection(&self) {
        if let Some(g) = self
            .layout
            .groups
            .iter()
            .find(|g| g.id == self.selected_group())
        {
            self.set(GROUP_NAME, &g.name);
        }
    }
    unsafe fn selected_items(&self, id: u16) -> Vec<Item> {
        let list = self.c(id);
        let n = SendMessageW(list, LB_GETSELCOUNT, 0, 0).max(0) as usize;
        let mut rows = vec![0i32; n.min(MAX_ITEMS)];
        let n =
            SendMessageW(list, LB_GETSELITEMS, rows.len(), rows.as_mut_ptr() as _).max(0) as usize;
        rows[..n]
            .iter()
            .filter_map(|row| self.layout.items.get(*row as usize).cloned())
            .collect()
    }
    unsafe fn refresh(&mut self) {
        let selected = self.selected_group();
        let groups = self.c(GROUPS);
        SendMessageW(groups, LB_RESETCONTENT, 0, 0);
        for (row, g) in self.layout.groups.iter().enumerate() {
            SendMessageW(
                groups,
                LB_ADDSTRING,
                0,
                wide(format!("{} ({})", g.name, g.items.len())).as_ptr() as _,
            );
            if g.id == selected {
                SendMessageW(groups, LB_SETCURSEL, row, 0);
            }
        }
        if SendMessageW(groups, LB_GETCURSEL, 0, 0) == LB_ERR as isize {
            SendMessageW(groups, LB_SETCURSEL, 0, 0);
        }
        for list_id in [ITEMS, AI_ITEMS] {
            let list = self.c(list_id);
            let was_visible = GetWindowLongPtrW(list, GWL_STYLE) as u32 & WS_VISIBLE != 0;
            if was_visible {
                SendMessageW(list, WM_SETREDRAW, 0, 0);
            }
            SendMessageW(list, LB_RESETCONTENT, 0, 0);
            for item in &self.layout.items {
                let group = self
                    .layout
                    .groups
                    .iter()
                    .find(|g| g.items.contains(&item.id))
                    .map(|g| g.name.as_str())
                    .unwrap_or("?");
                let text = if list_id == ITEMS {
                    format!("{}    ·    {}", item.name, group)
                } else {
                    format!("{}    ·    {}", item.name, item.category)
                };
                SendMessageW(list, LB_ADDSTRING, 0, wide(text).as_ptr() as _);
            }
            let longest = self
                .layout
                .items
                .iter()
                .map(|i| i.name.chars().count() + 20)
                .max()
                .unwrap_or(0);
            SendMessageW(
                list,
                LB_SETHORIZONTALEXTENT,
                scale((longest * 8) as i32, window_dpi(self.hwnd)) as _,
                0,
            );
            if was_visible {
                SendMessageW(list, WM_SETREDRAW, 1, 0);
                InvalidateRect(list, null(), 0);
            }
        }
        self.group_selection();
        self.update_actions();
        for (id, w) in &self.windows {
            if let Some(g) = self.layout.groups.iter().find(|g| g.id == *id) {
                SetWindowTextW(
                    w.hwnd,
                    wide(format!("{} — {} items", g.name, g.items.len())).as_ptr(),
                );
            }
        }
    }
    unsafe fn changed(&mut self) {
        self.dirty = true;
        self.plan = None;
        SetTimer(self.hwnd, SAVE_TIMER, 450, None);
    }
    unsafe fn flush_save(&mut self) -> Result<u64> {
        self.layout.validate()?;
        self.save_sequence += 1;
        self.worker.submit(Job::Save(
            self.save_sequence,
            self.layout.clone(),
            self.data.join("layout.json"),
        ))?;
        self.dirty = false;
        Ok(self.save_sequence)
    }
    unsafe fn command(&mut self, id: u16) -> Result<()> {
        trace(&format!("command {id}"));
        match id {
            AI_SETUP => {
                let visible = IsWindowVisible(self.hwnd) != 0;
                if visible {
                    SendMessageW(self.hwnd, WM_SETREDRAW, 0, 0);
                }
                self.ai_setup_open = !self.ai_setup_open;
                self.set(
                    AI_SETUP,
                    if self.ai_setup_open {
                        "Done"
                    } else {
                        "Connect AI"
                    },
                );
                self.position_controls();
                if visible {
                    SendMessageW(self.hwnd, WM_SETREDRAW, 1, 0);
                }
                RedrawWindow(
                    self.hwnd,
                    null(),
                    null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_FRAME,
                );
                return Ok(());
            }
            PAGE_LIBRARY | PAGE_AI | PAGE_SETTINGS => {
                self.switch_page((id - 100) as u8);
                return Ok(());
            }
            SHOW_GROUPS | TRAY_SHOW => {
                self.hidden = false;
                self.show_groups();
                return Ok(());
            }
            TRAY_HIDE => {
                self.hidden = true;
                self.show_groups();
                return Ok(());
            }
            TRAY_OPEN => {
                self.restore();
                return Ok(());
            }
            QUIT | TRAY_QUIT => {
                ensure!(
                    self.task == 0,
                    "Wait for the current operation before quitting"
                );
                KillTimer(self.hwnd, SAVE_TIMER);
                let sequence = self.flush_save()?;
                self.quitting = Some(sequence);
                self.status("Saving before exit...");
                return Ok(());
            }
            SELECT_ALL => {
                SendMessageW(self.c(AI_ITEMS), LB_SETSEL, 1, -1);
                return Ok(());
            }
            _ => {}
        }
        ensure!(
            self.task == 0 && self.quitting.is_none(),
            "Wait for the current background operation"
        );
        match id {
            BROWSE => {
                let mut info: BROWSEINFOW = zeroed();
                info.hwndOwner = self.hwnd;
                info.lpszTitle = w!("Choose the source folder. Importing does not move files.");
                info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
                let pidl = SHBrowseForFolderW(&info);
                if !pidl.is_null() {
                    let mut path = vec![0u16; 32768];
                    if SHGetPathFromIDListEx(pidl, path.as_mut_ptr(), path.len() as _, 0) != 0 {
                        let n = path.iter().position(|c| *c == 0).unwrap_or(0);
                        self.set(ROOT, &String::from_utf16_lossy(&path[..n]));
                        CoTaskMemFree(pidl.cast());
                        return self.command(SCAN);
                    }
                    CoTaskMemFree(pidl.cast());
                }
            }
            SCAN => {
                let root = PathBuf::from(self.text(ROOT));
                ensure!(root.is_absolute(), "Choose an absolute source folder");
                self.layout.settings.root = root.clone();
                self.changed();
                self.worker.submit(Job::Scan(root))?;
                self.task = 1;
                self.status("Importing folder metadata in the background...");
            }
            NEW_GROUP => {
                self.layout
                    .add_group("New group")
                    .context("Maximum of 40 groups reached")?;
                self.rebuild_groups()?;
                self.refresh();
                SendMessageW(
                    self.c(GROUPS),
                    LB_SETCURSEL,
                    self.layout.groups.len() - 1,
                    0,
                );
                self.group_selection();
                self.changed();
            }
            RENAME_GROUP => {
                let name = self.text(GROUP_NAME);
                ensure!(
                    !name.trim().is_empty()
                        && name.chars().count() <= 60
                        && !name.chars().any(char::is_control),
                    "Enter a group name of 1-60 characters"
                );
                let id = self.selected_group();
                if let Some(g) = self.layout.groups.iter_mut().find(|g| g.id == id) {
                    g.name = name;
                }
                self.refresh();
                self.changed();
            }
            DELETE_GROUP => {
                ensure!(self.layout.groups.len() > 1, "Keep at least one group");
                let id = self.selected_group();
                let items = self
                    .layout
                    .groups
                    .iter()
                    .find(|g| g.id == id)
                    .unwrap()
                    .items
                    .clone();
                self.layout.groups.retain(|g| g.id != id);
                self.layout.groups[0].items.extend(items);
                self.rebuild_groups()?;
                self.refresh();
                self.changed();
            }
            REMOVE => {
                let ids: Vec<_> = self.selected_items(ITEMS).iter().map(|i| i.id).collect();
                self.remove_items(&ids);
            }
            ASSIGN => {
                let target = self.selected_group();
                for item in self.selected_items(ITEMS) {
                    self.layout.assign(item.id, target);
                }
                self.refresh();
                self.invalidate_groups();
                self.changed();
            }
            RENAME_PROPOSAL => {
                let mut groups = ai::parse_content(&self.text(JSON), &self.scope)?;
                let row = SendMessageW(self.c(PROPOSALS), LB_GETCURSEL, 0, 0);
                ensure!(row >= 0, "Select a proposed group first");
                groups[row as usize].name = self.text(PROPOSAL_NAME);
                grouping::validate(&groups, &self.scope)?;
                self.set_suggestions(groups)?;
                self.plan = None;
            }
            ASSIGN_PROPOSAL => {
                let mut groups = ai::parse_content(&self.text(JSON), &self.scope)?;
                let row = SendMessageW(self.c(PROPOSALS), LB_GETCURSEL, 0, 0);
                ensure!(row >= 0, "Select a proposed group first");
                let selected = self.selected_items(AI_ITEMS);
                ensure!(
                    !selected.is_empty() && selected.iter().all(|i| self.scope.contains(i)),
                    "Select files included in this preview"
                );
                let ids: Vec<_> = selected.iter().map(|i| i.id).collect();
                for group in &mut groups {
                    group.item_ids.retain(|id| !ids.contains(id));
                }
                groups[row as usize].item_ids.extend(ids);
                groups.retain(|g| !g.item_ids.is_empty());
                grouping::validate(&groups, &self.scope)?;
                self.set_suggestions(groups)?;
                self.plan = None;
            }
            OFFLINE | REQUEST_AI => {
                self.scope = self.selected_items(AI_ITEMS);
                ensure!(!self.scope.is_empty(), "Select metadata items first");
                self.plan = None;
                if id == OFFLINE {
                    let groups = grouping::offline(&self.scope);
                    self.set_suggestions(groups)?;
                    self.status("Preview ready. Rename groups or adjust files, then choose Apply groups. Your files stay in place.");
                } else {
                    ensure!(
                        self.checked(CONSENT),
                        "Check the explicit metadata sharing consent first"
                    );
                    ensure!(
                        self.scope.len() <= crate::model::MAX_AI_ITEMS,
                        "Select at most 200 metadata items for AI"
                    );
                    self.worker.submit(Job::Ai {
                        base: self.layout.settings.ai_base_url.clone(),
                        model: self.layout.settings.ai_model.clone(),
                        key: self.layout.settings.protected_api_key.clone(),
                        items: self.scope.clone(),
                    })?;
                    self.task = 2;
                    self.status("Requesting metadata-only AI suggestions. Offline grouping remains available if the provider fails.");
                }
            }
            APPLY_VISUAL => {
                let groups = ai::parse_content(&self.text(JSON), &self.scope)?;
                grouping::apply_selected(&mut self.layout, &groups, &self.scope)?;
                self.rebuild_groups()?;
                self.refresh();
                self.changed();
                self.status("Visual groups applied. No files were moved.");
            }
            PREVIEW_MOVES => {
                ensure!(
                    self.checked(MOVE_OPT),
                    "Enable Create folders and move files before previewing"
                );
                let text = self.text(JSON);
                let groups = ai::parse_content(&text, &self.scope)?;
                self.plan_json = text;
                self.plan = None;
                self.worker.submit(Job::Preview(
                    self.layout.settings.root.clone(),
                    self.scope.clone(),
                    groups,
                ))?;
                self.task = 3;
                self.status("Validating every source, destination and content fingerprint...");
            }
            APPLY_MOVES => {
                ensure!(
                    self.checked(MOVE_OPT) && self.text(JSON) == self.plan_json,
                    "Options or JSON changed. Preview the moves again."
                );
                let plan = self.plan.clone().context("Preview file moves first")?;
                let journal = self.data.join("operation.json");
                if journal.exists() {
                    let old: Plan = persistence::load(&journal)?;
                    ensure!(
                        old.moves
                            .iter()
                            .all(|m| m.state == MoveState::Undone || m.state == MoveState::Pending),
                        "Undo or resolve the previous operation before starting another. Its journal is preserved."
                    );
                }
                if MessageBoxW(self.hwnd,wide(format!("Move {} files exactly as shown in the scrollable preview?\nNo existing file will be overwritten. Undo uses the saved operation journal.",plan.moves.len())).as_ptr(),w!("Apply file moves"),MB_YESNO|MB_ICONWARNING|MB_DEFBUTTON2)!=IDYES { return Ok(()); }
                self.worker.submit(Job::Apply(plan, journal))?;
                self.task = 4;
                self.plan = None;
                self.status("Moving verified files; progress is recorded in operation.json...");
            }
            UNDO => {
                self.worker
                    .submit(Job::Undo(self.data.join("operation.json")))?;
                self.task = 5;
                self.status(
                    "Recovering the journal and undoing completed moves without overwrites...",
                );
            }
            ACCENT => {
                let mut custom = [0u32; 16];
                let mut choose: CHOOSECOLORW = zeroed();
                choose.lStructSize = size_of::<CHOOSECOLORW>() as _;
                choose.hwndOwner = self.hwnd;
                choose.rgbResult = self.layout.settings.accent;
                choose.lpCustColors = custom.as_mut_ptr();
                choose.Flags = CC_FULLOPEN | CC_RGBINIT;
                if ChooseColorW(&mut choose) != 0 {
                    self.layout.settings.accent = choose.rgbResult;
                    self.invalidate_groups();
                    self.changed();
                }
            }
            SAVE_SETTINGS => {
                let base = self.text(BASE);
                ai::endpoint(&base)?;
                let model = self.text(MODEL);
                ensure!(
                    !model.trim().is_empty() && model.len() < 256,
                    "Enter a model name"
                );
                let percent = SendMessageW(self.c(OPACITY_SLIDER), TBM_GETPOS, 0, 0) as u32;
                ensure!((60..=100).contains(&percent), "Opacity must be 60–100%");
                let current_percent = (self.layout.settings.opacity as u32 * 100 + 127) / 255;
                let opacity = if percent == current_percent {
                    self.layout.settings.opacity
                } else {
                    ((percent * 255 + 50) / 100) as u8
                };
                let mut key = self.text(KEY).into_bytes();
                if !key.is_empty() {
                    self.layout.settings.protected_api_key = super::secrets::protect(&key)?;
                } else if self.checked(CLEAR_KEY) {
                    self.layout.settings.protected_api_key.clear();
                }
                key.fill(0);
                self.set(KEY, "");
                SendMessageW(self.c(CLEAR_KEY), BM_SETCHECK, BST_UNCHECKED as _, 0);
                let hide_on_fullscreen = self.checked(FULLSCREEN);
                let startup_update_check = self.checked(AUTO_CHECK);
                let s = &mut self.layout.settings;
                s.ai_base_url = base;
                s.ai_model = model;
                s.opacity = opacity;
                s.hide_on_fullscreen = hide_on_fullscreen;
                s.startup_update_check = startup_update_check;
                for w in self.windows.values() {
                    SetLayeredWindowAttributes(w.hwnd, 0, opacity, LWA_ALPHA);
                }
                self.changed();
                self.fullscreen_policy();
                self.status("Settings saved in the background. The API token stays protected by Windows DPAPI.");
            }
            CHECK => {
                self.worker.submit(Job::Check)?;
                self.task = 6;
                self.set_update("Checking the canonical GitHub repository...");
            }
            INSTALL => {
                let update = self
                    .available
                    .clone()
                    .context("Check for an available compatible release first")?;
                self.worker.submit(Job::Download(update))?;
                self.task = 7;
                self.set_update(
                    "Downloading update: 0%. The current executable and settings are preserved.",
                );
            }
            _ => {}
        }
        Ok(())
    }
    unsafe fn update_actions(&self) {
        let ready = self.task == 0 && self.quitting.is_none();
        let suggested = !self.text(JSON).is_empty();
        for control in &self.controls {
            if matches!(control.id, PAGE_LIBRARY | PAGE_AI | PAGE_SETTINGS) {
                SetWindowLongPtrW(
                    control.hwnd,
                    GWLP_USERDATA,
                    (control.id - 100 == self.page as u16) as isize,
                );
            }
            set_control_visible(control.hwnd, self.control_visible(control));
        }
        set_control_visible(
            self.c(ITEMS),
            self.page == 1 && !self.layout.items.is_empty(),
        );
        for (id, enabled) in [
            (
                ASSIGN,
                ready && SendMessageW(self.c(ITEMS), LB_GETSELCOUNT, 0, 0) > 0,
            ),
            (
                REMOVE,
                ready && SendMessageW(self.c(ITEMS), LB_GETSELCOUNT, 0, 0) > 0,
            ),
            (DELETE_GROUP, ready && self.layout.groups.len() > 1),
            (
                CLEAR_KEY,
                !self.layout.settings.protected_api_key.is_empty(),
            ),
            (INSTALL, ready && self.available.is_some()),
            (APPLY_VISUAL, ready && suggested),
            (RENAME_PROPOSAL, ready && suggested),
            (ASSIGN_PROPOSAL, ready && suggested),
            (OFFLINE, ready && !self.layout.items.is_empty()),
            (REQUEST_AI, ready && !self.layout.items.is_empty()),
            (PREVIEW_MOVES, ready && suggested && self.checked(MOVE_OPT)),
            (APPLY_MOVES, ready && self.plan.is_some()),
            (SCAN, ready && !self.text(ROOT).trim().is_empty()),
        ] {
            if (IsWindowEnabled(self.c(id)) != 0) != enabled {
                EnableWindow(self.c(id), enabled as _);
            }
        }
        for id in [PREVIEW_MOVES, APPLY_MOVES, MOVES] {
            set_control_visible(self.c(id), self.page == 2 && self.checked(MOVE_OPT));
        }
        let has_journal = self.data.join("operation.json").exists();
        set_control_visible(
            self.c(UNDO),
            self.page == 2 && (self.checked(MOVE_OPT) || has_journal),
        );
    }
    fn control_visible(&self, control: &Control) -> bool {
        if !(control.page == 0
            || control.page == self.page
            || (control.page == 4 && self.page == 3 && self.ai_setup_open))
        {
            return false;
        }
        match control.id {
            ITEMS => self.page == 1 && !self.layout.items.is_empty(),
            PREVIEW_MOVES | APPLY_MOVES | MOVES => {
                self.page == 2 && unsafe { self.checked(MOVE_OPT) }
            }
            UNDO => {
                self.page == 2
                    && (unsafe { self.checked(MOVE_OPT) }
                        || self.data.join("operation.json").exists())
            }
            _ => true,
        }
    }
    unsafe fn remove_items(&mut self, ids: &[u64]) {
        self.layout.items.retain(|i| !ids.contains(&i.id));
        for g in &mut self.layout.groups {
            g.items.retain(|i| !ids.contains(i));
        }
        self.refresh();
        self.invalidate_groups();
        self.changed();
        self.status("References removed. Original files were left in place.");
    }
    unsafe fn drop_files(&mut self, drop: HDROP, group: u64) {
        let count = DragQueryFileW(drop, u32::MAX, null_mut(), 0);
        let mut paths = vec![];
        if count as usize <= MAX_ITEMS && self.task == 0 {
            for index in 0..count {
                let n = DragQueryFileW(drop, index, null_mut(), 0);
                let mut buf = vec![0u16; n as usize + 1];
                DragQueryFileW(drop, index, buf.as_mut_ptr(), buf.len() as _);
                paths.push(PathBuf::from(String::from_utf16_lossy(&buf[..n as usize])));
            }
        }
        DragFinish(drop);
        if paths.is_empty() {
            self.status("Drop rejected: wait for the current task or drop at most 2000 items.");
            return;
        }
        match self.worker.submit(Job::Import(paths, group)) {
            Ok(()) => {
                self.task = 1;
                self.status("Validating dropped references...");
            }
            Err(e) => self.error(e),
        }
    }
    unsafe fn worker_events(&mut self) {
        let mut changed = false;
        while let Ok(event) = self.worker.receiver.try_recv() {
            changed = true;
            match event {
                Event::Saved(sequence, result) => match result {
                    Ok(()) => {
                        if self.quitting == Some(sequence) {
                            if let Some(prepared) = self.pending_install.take() {
                                match self.worker.submit(Job::Install(prepared)) {
                                    Ok(()) => {
                                        self.task = 8;
                                        self.set_update(
                                            "Settings saved. Preparing restart helper...",
                                        );
                                    }
                                    Err(e) => {
                                        self.quitting = None;
                                        self.error(e);
                                    }
                                }
                            } else {
                                DestroyWindow(self.hwnd);
                            }
                        }
                    }
                    Err(e) => {
                        self.quitting = None;
                        self.pending_install = None;
                        self.dirty = true;
                        self.error(e);
                    }
                },
                Event::Scanned(result) => {
                    self.task = 0;
                    let group = self.selected_group();
                    self.import_result(group, result, true);
                }
                Event::Imported(group, result) => {
                    self.task = 0;
                    self.import_result(group, result, false);
                }
                Event::Suggested(result) => {
                    self.task = 0;
                    match result {
                        Ok(groups) => {
                            if let Err(error) = self.set_suggestions(groups) {
                                self.error(error);
                            }
                            self.status(
                                "AI suggestion ready. Review or edit every group before applying.",
                            );
                        }
                        Err(e) => self.error(e),
                    }
                }
                Event::Previewed(result) => {
                    self.task = 0;
                    match result {
                        Ok(plan) => {
                            let text = plan
                                .moves
                                .iter()
                                .map(|m| {
                                    format!(
                                        "{}  ->  {}",
                                        m.source.display(),
                                        m.destination.display()
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\r\n");
                            self.set(MOVES, &text);
                            self.plan = Some(plan);
                            self.status("Move preview ready. Scroll to review every source and destination before Apply.");
                        }
                        Err(e) => self.error(e),
                    }
                }
                Event::Applied(result) => {
                    self.task = 0;
                    match result {
                        Ok(plan) => {
                            self.sync_paths(&plan);
                            self.status("File moves completed and journaled. Undo is available until another operation replaces the journal.");
                        }
                        Err(e) => {
                            self.error(e);
                            self.set(MOVES,"Operation stopped. Some earlier moves may have completed. Use Undo last file operation to recover the persisted journal.");
                        }
                    }
                }
                Event::Undone(result) => {
                    self.task = 0;
                    match result {
                        Ok(plan) => {
                            self.sync_paths(&plan);
                            self.status("Completed moves were undone. Changed files and occupied paths are never overwritten.");
                        }
                        Err(e) => self.error(e),
                    }
                }
                Event::Recovered(result) => match result {
                    Ok(plan) => {
                        self.sync_paths(&plan);
                        self.status("Operation journal reconciled. Use Undo last file operation to reverse completed moves.");
                    }
                    Err(e) => {
                        self.error(e.context("The operation journal needs attention; preserve it and resolve conflicts before further moves"));
                    }
                },
                Event::Checked(result) => {
                    if self.task == 6 {
                        self.task = 0;
                    }
                    match result {
                        Ok(Some(update)) => {
                            self.set_update(&format!("Version {} is available. Download and install will close and restart this application.",update.version));
                            self.status(&format!(
                                "DesktopOrganizer {} is available in Settings / Updates.",
                                update.version
                            ));
                            self.available = Some(update);
                            self.notify_update();
                        }
                        Ok(None) => {
                            self.available = None;
                            self.set_update(
                                "The installed version is up to date on the stable channel.",
                            );
                        }
                        Err(e) => {
                            self.available = None;
                            self.set_update(&format!("Update check failed: {e:#}"));
                        }
                    }
                }
                Event::Progress(n, total) => {
                    self.set_update(&format!(
                        "Downloading update: {}% ({n} / {total} bytes)",
                        n * 100 / total.max(1)
                    ));
                }
                Event::Downloaded(result) => {
                    self.task = 0;
                    match result {
                        Ok(stage) => {
                            self.pending_install = Some(stage);
                            match self.flush_save() {
                                Ok(sequence) => {
                                    self.quitting = Some(sequence);
                                    self.set_update(
                                        "Verified update downloaded. Saving and restarting...",
                                    );
                                }
                                Err(e) => {
                                    self.pending_install = None;
                                    self.error(e);
                                }
                            }
                        }
                        Err(e) => {
                            self.set_update(&format!("Update download failed: {e:#}"));
                        }
                    }
                }
                Event::Installed(result) => {
                    self.task = 0;
                    match result {
                        Ok(()) => {
                            DestroyWindow(self.hwnd);
                        }
                        Err(e) => {
                            self.quitting = None;
                            self.error(e);
                        }
                    }
                }
            }
        }
        if changed {
            self.update_actions();
        }
    }
    unsafe fn sync_paths(&mut self, plan: &Plan) {
        for m in &plan.moves {
            if let Some(item) = self
                .layout
                .items
                .iter_mut()
                .find(|i| i.id == m.item_id && (i.path == m.source || i.path == m.destination))
            {
                item.path = if m.state == MoveState::Done {
                    m.destination.clone()
                } else {
                    m.source.clone()
                };
                item.name = item
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
            }
        }
        self.refresh();
        self.invalidate_groups();
        self.changed();
    }
    unsafe fn import_result(
        &mut self,
        group: u64,
        result: Result<Vec<(PathBuf, bool)>>,
        preserve_groups: bool,
    ) {
        match result {
            Ok(entries) => {
                let mut count = 0;
                for (path, dir) in entries {
                    if if preserve_groups {
                        self.layout.refresh_reference(path, dir, group)
                    } else {
                        self.layout.import(path, dir, group)
                    } {
                        count += 1;
                    }
                }
                self.refresh();
                self.invalidate_groups();
                self.changed();
                self.status(&format!("{count} items ready. Originals stay in place."));
            }
            Err(e) => self.error(e),
        }
    }
    unsafe fn restore(&mut self) {
        ShowWindow(
            self.hwnd,
            if IsIconic(self.hwnd) != 0 {
                SW_RESTORE
            } else {
                SW_SHOW
            },
        );
        SetForegroundWindow(self.hwnd);
    }
    unsafe fn show_groups(&self) {
        for w in self.windows.values() {
            ShowWindow(
                w.hwnd,
                if self.hidden || self.fullscreen {
                    SW_HIDE
                } else {
                    SW_SHOWNOACTIVATE
                },
            );
            if !self.hidden && !self.fullscreen {
                SetWindowPos(
                    w.hwnd,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
    unsafe fn register_tray(&mut self) {
        match self.tray(true) {
            Ok(()) => self.tray_available = true,
            Err(error) => {
                self.tray_available = false;
                self.restore();
                self.error(error);
            }
        }
    }
    unsafe fn tray(&self, add: bool) -> Result<()> {
        let mut icon: NOTIFYICONDATAW = zeroed();
        icon.cbSize = size_of::<NOTIFYICONDATAW>() as _;
        icon.hWnd = self.hwnd;
        icon.uID = 1;
        icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        icon.uCallbackMessage = TRAY;
        icon.hIcon = self.small_icon;
        let tip = wide("DesktopOrganizer — Click for settings");
        icon.szTip[..tip.len()].copy_from_slice(&tip);
        ensure!(
            Shell_NotifyIconW(if add { NIM_ADD } else { NIM_DELETE }, &icon) != 0,
            "Windows tray is unavailable; keep the settings window open"
        );
        Ok(())
    }
    unsafe fn notify_update(&self) {
        let Some(update) = &self.available else {
            return;
        };
        let mut icon: NOTIFYICONDATAW = zeroed();
        icon.cbSize = size_of::<NOTIFYICONDATAW>() as _;
        icon.hWnd = self.hwnd;
        icon.uID = 1;
        icon.uFlags = NIF_INFO;
        let title = wide(format!("DesktopOrganizer {} is available", update.version));
        let body = wide(
            "Open Settings / Updates to review, download and install. Nothing is downloaded automatically.",
        );
        let n = title.len().min(icon.szInfoTitle.len() - 1);
        icon.szInfoTitle[..n].copy_from_slice(&title[..n]);
        let n = body.len().min(icon.szInfo.len() - 1);
        icon.szInfo[..n].copy_from_slice(&body[..n]);
        icon.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
        Shell_NotifyIconW(NIM_MODIFY, &icon);
    }
    unsafe fn tray_menu(&mut self) {
        let menu = CreatePopupMenu();
        for (id, text) in [
            (TRAY_OPEN, "Open library / settings"),
            (TRAY_SHOW, "Show groups"),
            (TRAY_HIDE, "Hide groups"),
            (TRAY_QUIT, "Save and quit"),
        ] {
            AppendMenuW(menu, MF_STRING, id as _, wide(text).as_ptr());
        }
        let mut point = POINT::default();
        GetCursorPos(&mut point);
        SetForegroundWindow(self.hwnd);
        let id = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            self.hwnd,
            null(),
        ) as u16;
        DestroyMenu(menu);
        if let Err(e) = self.command(id) {
            self.error(e);
        }
        PostMessageW(self.hwnd, WM_NULL, 0, 0);
    }
    unsafe fn group_menu(&mut self, id: u64) {
        let menu = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, 1, w!("Collapse / expand"));
        AppendMenuW(menu, MF_STRING, 2, w!("Open library / settings"));
        AppendMenuW(menu, MF_STRING, 3, w!("Hide all groups"));
        let mut point = POINT::default();
        GetCursorPos(&mut point);
        let window = self.windows[&id].hwnd;
        SetForegroundWindow(window);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            window,
            null(),
        );
        DestroyMenu(menu);
        match command {
            1 => self.collapse(id),
            2 => self.restore(),
            3 => {
                self.hidden = true;
                self.show_groups();
            }
            _ => {}
        }
    }
    unsafe fn fullscreen_policy(&mut self) {
        let foreground = GetForegroundWindow();
        let mut class = [0u16; 80];
        let n = GetClassNameW(foreground, class.as_mut_ptr(), 80);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        let mut full = false;
        if self.layout.settings.hide_on_fullscreen
            && !foreground.is_null()
            && !self.windows.values().any(|w| w.hwnd == foreground)
            && foreground != self.hwnd
            && !matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd")
        {
            let mut rect = RECT::default();
            let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as _;
            if GetWindowRect(foreground, &mut rect) != 0
                && GetMonitorInfoW(monitor, &mut info) != 0
                && IsIconic(foreground) == 0
            {
                full = rect.left <= info.rcMonitor.left
                    && rect.top <= info.rcMonitor.top
                    && rect.right >= info.rcMonitor.right
                    && rect.bottom >= info.rcMonitor.bottom;
            }
        }
        if full != self.fullscreen {
            self.fullscreen = full;
            self.show_groups();
        }
    }
    unsafe fn rebuild_groups(&mut self) -> Result<()> {
        let old = std::mem::take(&mut self.windows);
        for w in old.values() {
            DestroyWindow(w.hwnd);
        }
        for group in &self.layout.groups {
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                GROUP_CLASS,
                wide(format!("{} — {} items", group.name, group.items.len())).as_ptr(),
                WS_POPUP | WS_THICKFRAME | WS_CLIPCHILDREN,
                0,
                0,
                360,
                340,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            );
            ensure!(!hwnd.is_null(), "Could not create a group window");
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, group.id as _);
            DragAcceptFiles(hwnd, 1);
            SetLayeredWindowAttributes(hwnd, 0, self.layout.settings.opacity, LWA_ALPHA);
            place(hwnd, &group.placement, group.collapsed);
            self.windows.insert(
                group.id,
                GroupWindow {
                    hwnd,
                    scroll: 0,
                    selected: None,
                },
            );
        }
        self.show_groups();
        Ok(())
    }
    unsafe fn invalidate_groups(&self) {
        for w in self.windows.values() {
            InvalidateRect(w.hwnd, null(), 0);
        }
    }
    unsafe fn capture_placement(&mut self, id: u64) {
        let Some(window) = self.windows.get(&id) else {
            return;
        };
        let mut rect = RECT::default();
        GetWindowRect(window.hwnd, &mut rect);
        let dpi = window_dpi(window.hwnd).max(96);
        let monitor = MonitorFromWindow(window.hwnd, MONITOR_DEFAULTTONEAREST);
        let info = monitor_info(monitor);
        if let Some(group) = self.layout.groups.iter_mut().find(|g| g.id == id) {
            group.placement.monitor = device_name(&info);
            group.placement.x = unscale(rect.left - info.monitorInfo.rcWork.left, dpi);
            group.placement.y = unscale(rect.top - info.monitorInfo.rcWork.top, dpi);
            group.placement.width = unscale(rect.right - rect.left, dpi).max(240);
            if !group.collapsed {
                group.placement.height = unscale(rect.bottom - rect.top, dpi).max(150);
            }
        }
    }
    unsafe fn recover_positions(&mut self) {
        for g in &self.layout.groups {
            if let Some(w) = self.windows.get(&g.id) {
                place(w.hwnd, &g.placement, g.collapsed);
            }
        }
    }
    unsafe fn collapse(&mut self, id: u64) {
        if let Some(g) = self.layout.groups.iter_mut().find(|g| g.id == id) {
            g.collapsed = !g.collapsed;
            if let Some(w) = self.windows.get(&id) {
                place(w.hwnd, &g.placement, g.collapsed);
            }
            self.changed();
        }
    }
    unsafe fn hit_item(&self, id: u64, lp: LPARAM) -> Option<u64> {
        let window = self.windows.get(&id)?;
        let group = self.layout.groups.iter().find(|g| g.id == id)?;
        if group.collapsed {
            return None;
        }
        let dpi = window_dpi(window.hwnd).max(96);
        let mut r = RECT::default();
        GetClientRect(window.hwnd, &mut r);
        let width = unscale(r.right, dpi);
        let index = group_item_index(
            unscale(signed_low(lp), dpi),
            unscale(signed_high(lp), dpi),
            width,
            window.scroll,
        )?;
        group.items.get(index).copied()
    }
    unsafe fn open_item(&self, id: u64) {
        if let Some(item) = self.layout.items.iter().find(|i| i.id == id) {
            let result = ShellExecuteW(
                self.hwnd,
                w!("open"),
                wide(&item.path).as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            );
            if result as isize <= 32 {
                self.status("Windows could not open this item. The target may have moved, or no associated application is installed.");
            }
        }
    }
    unsafe fn paint_group(&mut self, id: u64) {
        let Some(window) = self.windows.get(&id) else {
            return;
        };
        let hwnd = window.hwnd;
        let mut ps: PAINTSTRUCT = zeroed();
        let output = BeginPaint(hwnd, &mut ps);
        self.render_group(id, output);
        EndPaint(hwnd, &ps);
    }
    unsafe fn render_group(&mut self, id: u64, output: HDC) {
        GROUP_PAINTS.fetch_add(1, Ordering::Relaxed);
        let Some(window) = self.windows.get_mut(&id) else {
            return;
        };
        let hwnd = window.hwnd;
        let selected = window.selected;
        let Some(group) = self.layout.groups.iter().find(|g| g.id == id) else {
            return;
        };
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        let dpi = window_dpi(hwnd).max(96);
        let dc = CreateCompatibleDC(output);
        let bitmap = CreateCompatibleBitmap(output, r.right.max(1), r.bottom.max(1));
        let old_bitmap = SelectObject(dc, bitmap);
        fill(dc, r, 0x00251d16);
        SetBkMode(dc, TRANSPARENT as _);
        let font = CreateFontW(
            -scale(14, dpi),
            0,
            0,
            0,
            FW_NORMAL as _,
            0,
            0,
            0,
            DEFAULT_CHARSET as _,
            OUT_DEFAULT_PRECIS as _,
            CLIP_DEFAULT_PRECIS as _,
            CLEARTYPE_QUALITY as _,
            DEFAULT_PITCH as _,
            w!("Segoe UI"),
        );
        let old_font = SelectObject(dc, font);
        let width = unscale(r.right, dpi);
        let height = unscale(r.bottom, dpi);
        let cols = ((width - 24) / 100).max(1);
        let rows = (group.items.len() as i32 + cols - 1) / cols;
        window.scroll = window.scroll.clamp(0, (rows * 110 - (height - 86)).max(0));
        theme::heading(
            dc,
            &group.name,
            RECT {
                left: scale(16, dpi),
                top: scale(14, dpi),
                right: r.right - scale(16, dpi),
                bottom: scale(44, dpi),
            },
            20,
            theme::rgb(242, 246, 252),
            true,
            dpi,
        );
        if !group.collapsed {
            draw(
                dc,
                &format!(
                    "{} {}  ·  Drag to rearrange",
                    group.items.len(),
                    if group.items.len() == 1 {
                        "file"
                    } else {
                        "files"
                    }
                ),
                RECT {
                    left: scale(16, dpi),
                    top: scale(47, dpi),
                    right: r.right - scale(12, dpi),
                    bottom: scale(70, dpi),
                },
                theme::rgb(154, 172, 195),
                DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
        if group.items.is_empty() && !group.collapsed {
            draw(
                dc,
                "Your files stay where they are.\nDrop files to add visual references.",
                RECT {
                    left: scale(20, dpi),
                    top: scale(90, dpi),
                    right: r.right - scale(20, dpi),
                    bottom: r.bottom,
                },
                0x00ded7c9,
                DT_WORDBREAK,
            );
        }
        for (index, item_id) in group.items.iter().enumerate() {
            if group.collapsed {
                break;
            }
            let Some(item) = self.layout.items.iter().find(|i| i.id == *item_id) else {
                continue;
            };
            let x = 12 + index as i32 % cols * 100;
            let y = 80 + index as i32 / cols * 110 - window.scroll;
            if y + 102 < 80 || y > height {
                continue;
            }
            let tile = RECT {
                left: scale(x, dpi),
                top: scale(y, dpi),
                right: scale(x + 92, dpi),
                bottom: scale(y + 102, dpi),
            };
            // Clip tiles below the fixed instruction strip, including while scrolling.
            SaveDC(dc);
            IntersectClipRect(dc, 0, scale(80, dpi), r.right, r.bottom);
            theme::rounded(
                dc,
                tile,
                if selected == Some(*item_id) {
                    0x00604c30
                } else {
                    theme::rgb(34, 46, 62)
                },
                theme::rgb(45, 60, 79),
                scale(10, dpi),
            );
            let paper = RECT {
                left: tile.left + scale(32, dpi),
                top: tile.top + scale(12, dpi),
                right: tile.left + scale(60, dpi),
                bottom: tile.top + scale(45, dpi),
            };
            theme::rounded(
                dc,
                paper,
                theme::rgb(219, 232, 250),
                theme::rgb(219, 232, 250),
                scale(4, dpi),
            );
            for line in 0..3 {
                fill(
                    dc,
                    RECT {
                        left: paper.left + scale(6, dpi),
                        top: paper.top + scale(9 + line * 6, dpi),
                        right: paper.right - scale(6, dpi),
                        bottom: paper.top + scale(11 + line * 6, dpi),
                    },
                    self.layout.settings.accent,
                );
            }
            draw(
                dc,
                &item.name,
                RECT {
                    left: tile.left + scale(4, dpi),
                    top: tile.top + scale(54, dpi),
                    right: tile.right - scale(4, dpi),
                    bottom: tile.bottom - scale(3, dpi),
                },
                0x00f7f0e8,
                DT_CENTER | DT_WORDBREAK | DT_END_ELLIPSIS,
            );
            RestoreDC(dc, -1);
        }
        BitBlt(output, 0, 0, r.right, r.bottom, dc, 0, 0, SRCCOPY);
        SelectObject(dc, old_font);
        DeleteObject(font);
        SelectObject(dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
    }
}
fn group_item_index(x: i32, y: i32, width: i32, scroll: i32) -> Option<usize> {
    if y < 80 {
        return None;
    }
    let cols = ((width - 24) / 100).max(1);
    let x = x - 12;
    let y = y - 80 + scroll;
    if x < 0 || y < 0 || x >= cols * 100 || x % 100 >= 92 || y % 110 >= 102 {
        return None;
    }
    Some((y / 110 * cols + x / 100) as usize)
}
unsafe fn fill(dc: HDC, rect: RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    FillRect(dc, &rect, brush);
    DeleteObject(brush);
}
unsafe fn set_control_visible(hwnd: HWND, visible: bool) {
    if (GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0) != visible {
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_NOREDRAW
                | if visible {
                    SWP_SHOWWINDOW
                } else {
                    SWP_HIDEWINDOW
                },
        );
    }
}
unsafe fn draw(dc: HDC, text: &str, mut rect: RECT, color: u32, flags: u32) {
    SetTextColor(dc, color);
    let text = wide(text);
    DrawTextW(dc, text.as_ptr(), (text.len() - 1) as _, &mut rect, flags);
}
unsafe fn monitor_info(monitor: HMONITOR) -> MONITORINFOEXW {
    let mut info: MONITORINFOEXW = zeroed();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as _;
    GetMonitorInfoW(monitor, &mut info as *mut _ as _);
    info
}
fn device_name(info: &MONITORINFOEXW) -> String {
    let n = info
        .szDevice
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(info.szDevice.len());
    String::from_utf16_lossy(&info.szDevice[..n])
}
unsafe extern "system" fn find_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let pair = &mut *(data as *mut (String, HMONITOR));
    if device_name(&monitor_info(monitor)) == pair.0 {
        pair.1 = monitor;
        return 0;
    }
    1
}
unsafe fn place(hwnd: HWND, placement: &Placement, collapsed: bool) {
    let mut pair: (String, HMONITOR) = (placement.monitor.clone(), null_mut());
    EnumDisplayMonitors(
        null_mut(),
        null(),
        Some(find_monitor),
        &mut pair as *mut _ as _,
    );
    let monitor = if pair.1.is_null() {
        MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY)
    } else {
        pair.1
    };
    let info = monitor_info(monitor);
    let area = info.monitorInfo.rcWork;
    let mut dx = 96;
    let mut dy = 96;
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    let width = scale(placement.width.clamp(240, 1800), dx)
        .min(area.right - area.left)
        .max(120);
    let height = scale(
        if collapsed {
            90
        } else {
            placement.height.clamp(150, 1600)
        },
        dy,
    )
    .min(area.bottom - area.top)
    .max(60);
    let x =
        (area.left + scale(placement.x, dx)).clamp(area.left, (area.right - width).max(area.left));
    let y =
        (area.top + scale(placement.y, dy)).clamp(area.top, (area.bottom - height).max(area.top));
    SetWindowPos(
        hwnd,
        null_mut(),
        x,
        y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    InvalidateRect(hwnd, null(), 0);
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    #[test]
    fn group_pointer_mapping_matches_rows_gaps_header_and_scroll() {
        assert_eq!(group_item_index(20, 90, 360, 0), Some(0));
        assert_eq!(group_item_index(220, 200, 360, 0), Some(5));
        assert_eq!(group_item_index(20, 90, 360, 110), Some(3));
        assert_eq!(group_item_index(220, 50, 360, 110), None);
        assert_eq!(group_item_index(106, 90, 360, 0), None);
        assert_eq!(group_item_index(20, 184, 360, 0), None);
        assert_eq!(group_item_index(220, 200, 240, 0), None);
    }
}
