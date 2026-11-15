//! A monitor cell editor backed by the same persistent group placement model.
use super::*;
use crate::regions::{COLUMNS, ROWS, Region};
const CLASS: PCWSTR = w!("DesktopOrganizer.Arrangement.v1");
const CATEGORY: i32 = 1;
const MONITOR: i32 = 2;
const AUTO: i32 = 3;
const CLEAR: i32 = 4;
const APPLY: i32 = ARRANGE_APPLY as i32;
#[derive(Default)]
pub(super) struct Editor {
    pub window: HWND,
}
struct Entry {
    id: u64,
    name: String,
    monitor: String,
    region: Option<Region>,
}
struct Monitor {
    device: String,
    width: i32,
    height: i32,
    dpi: u32,
}
struct State {
    entries: Vec<Entry>,
    monitors: Vec<Monitor>,
    selected: usize,
    monitor: usize,
    drag: Option<(u8, u8)>,
    pending: Option<Region>,
    note: String,
}
impl Editor {
    pub unsafe fn show(&mut self, owner: HWND, font: HFONT, layout: &Layout) {
        unsafe {
            if !self.window.is_null() {
                DestroyWindow(self.window);
            }
            let mut monitors: Vec<Monitor> = vec![];
            unsafe extern "system" fn enumerate(
                handle: HMONITOR,
                _: HDC,
                _: *mut RECT,
                param: LPARAM,
            ) -> BOOL {
                unsafe {
                    let info = monitor_info(handle);
                    let mut x = 96;
                    let mut y = 96;
                    GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut x, &mut y);
                    (*(param as *mut Vec<Monitor>)).push(Monitor {
                        device: device_name(&info),
                        width: info.monitorInfo.rcWork.right - info.monitorInfo.rcWork.left,
                        height: info.monitorInfo.rcWork.bottom - info.monitorInfo.rcWork.top,
                        dpi: x,
                    });
                    1
                }
            }
            EnumDisplayMonitors(
                null_mut(),
                null(),
                Some(enumerate),
                &mut monitors as *mut Vec<Monitor> as isize,
            );
            if monitors.is_empty() {
                return;
            }
            let initial = Box::new(State {
                entries: layout
                    .groups
                    .iter()
                    .filter(|g| g.on_screen)
                    .map(|g| Entry {
                        id: g.id,
                        name: g.name.clone(),
                        monitor: if monitors.iter().any(|m| m.device == g.placement.monitor) {
                            g.placement.monitor.clone()
                        } else {
                            monitors[0].device.clone()
                        },
                        region: g.placement.region,
                    })
                    .collect(),
                monitors,
                selected: 0,
                monitor: 0,
                drag: None,
                pending: None,
                note: tr("Select a category, then drag across the cells.").into(),
            });
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: GetModuleHandleW(null()),
                lpszClassName: CLASS,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                ..zeroed()
            };
            RegisterClassW(&class);
            self.window = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_COMPOSITED,
                CLASS,
                wide(tr("Arrange categories on screen")).as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                940,
                740,
                owner,
                null_mut(),
                GetModuleHandleW(null()),
                Box::into_raw(initial).cast(),
            );
            if self.window.is_null() {
                return;
            }
            for (id, class, style, caption) in [
                (
                    CATEGORY,
                    w!("COMBOBOX"),
                    CBS_DROPDOWNLIST as u32
                        | CBS_OWNERDRAWFIXED as u32
                        | CBS_HASSTRINGS as u32
                        | WS_VSCROLL,
                    "",
                ),
                (
                    MONITOR,
                    w!("COMBOBOX"),
                    CBS_DROPDOWNLIST as u32 | CBS_OWNERDRAWFIXED as u32 | CBS_HASSTRINGS as u32,
                    "",
                ),
                (AUTO, w!("BUTTON"), BS_OWNERDRAW as u32, "Auto arrange"),
                (CLEAR, w!("BUTTON"), BS_OWNERDRAW as u32, "Clear region"),
                (
                    APPLY,
                    w!("BUTTON"),
                    BS_OWNERDRAW as u32,
                    "Save & show groups",
                ),
            ] {
                let child = CreateWindowExW(
                    0,
                    class,
                    wide(tr(caption)).as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | style,
                    0,
                    0,
                    0,
                    0,
                    self.window,
                    id as usize as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                );
                SendMessageW(child, WM_SETFONT, font as usize, 0);
                style_dialog_control(child);
            }
            if let Some(state) = state(self.window) {
                for entry in &state.entries {
                    SendMessageW(
                        GetDlgItem(self.window, CATEGORY),
                        CB_ADDSTRING,
                        0,
                        wide(&entry.name).as_ptr() as isize,
                    );
                }
                for (index, monitor) in state.monitors.iter().enumerate() {
                    SendMessageW(
                        GetDlgItem(self.window, MONITOR),
                        CB_ADDSTRING,
                        0,
                        wide(format!(
                            "{} · {} × {}",
                            index + 1,
                            monitor.width,
                            monitor.height
                        ))
                        .as_ptr() as isize,
                    );
                }
                SendMessageW(GetDlgItem(self.window, CATEGORY), CB_SETCURSEL, 0, 0);
                state.monitor = state
                    .entries
                    .first()
                    .and_then(|entry| {
                        state
                            .monitors
                            .iter()
                            .position(|m| m.device == entry.monitor)
                    })
                    .unwrap_or(0);
                SendMessageW(
                    GetDlgItem(self.window, MONITOR),
                    CB_SETCURSEL,
                    state.monitor,
                    0,
                );
                if state.entries.iter().all(|e| e.region.is_none()) {
                    auto_arrange(state);
                }
            }
            position(self.window);
            if CAPTURE_DPI.load(Ordering::Relaxed) == 0 {
                ShowWindow(self.window, SW_SHOWNORMAL);
                SetForegroundWindow(self.window);
            }
        }
    }
    pub unsafe fn hide(&self) {
        unsafe {
            ShowWindow(self.window, SW_HIDE);
        }
    }
    pub unsafe fn apply(&self, layout: &mut Layout) -> Result<()> {
        unsafe {
            let state = state(self.window).context("Screen arrangement is unavailable")?;
            ensure!(
                state.entries.len() == layout.groups.iter().filter(|g| g.on_screen).count()
                    && state.entries.iter().all(|entry| layout
                        .groups
                        .iter()
                        .any(|g| g.on_screen && g.id == entry.id && g.name == entry.name)),
                tr("Categories changed. Reopen the region editor before saving.")
            );
            ensure!(
                state.entries.iter().all(|e| e.region.is_some()),
                tr("Some categories need a region. Use free cells, another monitor, or folders.")
            );
            for entry in &state.entries {
                if let Some(group) = layout.groups.iter_mut().find(|g| g.id == entry.id) {
                    group.placement.monitor = entry.monitor.clone();
                    group.placement.region = entry.region;
                    group.collapsed = false;
                }
            }
            layout.validate()?;
            Ok(())
        }
    }
}
unsafe fn state<'a>(hwnd: HWND) -> Option<&'a mut State> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State).as_mut() }
}
fn auto_arrange(state: &mut State) {
    for entry in &mut state.entries {
        entry.region = None;
    }
    let mut index = 0;
    for monitor in &state.monitors {
        let minimum = fitted(
            Region {
                left: 0,
                top: 0,
                columns: 2,
                rows: 1,
            },
            monitor,
        );
        for row in (0..ROWS).step_by(minimum.rows as usize) {
            for col in (0..COLUMNS).step_by(minimum.columns as usize) {
                if col + minimum.columns > COLUMNS || row + minimum.rows > ROWS {
                    continue;
                }
                let Some(entry) = state.entries.get_mut(index) else {
                    break;
                };
                entry.monitor = monitor.device.clone();
                entry.region = Some(Region {
                    left: col,
                    top: row,
                    ..minimum
                });
                index += 1;
            }
        }
    }
    state.note = if index < state.entries.len() {
        tr("Some categories need a region. Use free cells, another monitor, or folders.")
    } else {
        tr("Regions prepared. Adjust them, then save.")
    }
    .into();
}
unsafe fn canvas(hwnd: HWND) -> RECT {
    unsafe {
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        let d = window_dpi(hwnd);
        RECT {
            left: scale(24, d),
            top: scale(158, d),
            right: r.right - scale(24, d),
            bottom: r.bottom - scale(100, d),
        }
    }
}
unsafe fn position(hwnd: HWND) {
    unsafe {
        AUX_LAYOUTS.fetch_add(1, Ordering::Relaxed);
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        let s = |v| scale(v, window_dpi(hwnd));
        for (id, x, y, w, h) in [
            (CATEGORY, s(24), s(66), (r.right - s(72)) / 2, s(220)),
            (
                MONITOR,
                (r.right + s(24)) / 2,
                s(66),
                (r.right - s(72)) / 2,
                s(220),
            ),
            (AUTO, s(24), r.bottom - s(52), s(190), s(34)),
            (CLEAR, s(224), r.bottom - s(52), s(180), s(34)),
            (APPLY, r.right - s(274), r.bottom - s(52), s(250), s(34)),
        ] {
            SetWindowPos(
                GetDlgItem(hwnd, id),
                null_mut(),
                x,
                y,
                w,
                h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
unsafe fn cell(hwnd: HWND, lp: LPARAM) -> Option<(u8, u8)> {
    unsafe {
        let r = canvas(hwnd);
        let x = signed_low(lp);
        let y = signed_high(lp);
        if x < r.left || x >= r.right || y < r.top || y >= r.bottom {
            return None;
        }
        Some((
            ((x - r.left) * COLUMNS as i32 / (r.right - r.left)) as u8,
            ((y - r.top) * ROWS as i32 / (r.bottom - r.top)) as u8,
        ))
    }
}
fn fitted(mut region: Region, monitor: &Monitor) -> Region {
    let min_columns = ((280 * monitor.dpi as i32 / 96 * COLUMNS as i32 + monitor.width - 1)
        / monitor.width)
        .clamp(2, COLUMNS as i32) as u8;
    let min_rows = ((240 * monitor.dpi as i32 / 96 * ROWS as i32 + monitor.height - 1)
        / monitor.height)
        .clamp(1, ROWS as i32) as u8;
    region.columns = region.columns.max(min_columns);
    region.rows = region.rows.max(min_rows);
    region.left = region.left.min(COLUMNS - region.columns);
    region.top = region.top.min(ROWS - region.rows);
    region
}
fn assign(state: &mut State, region: Region) {
    if state.entries.is_empty() {
        return;
    }
    let monitor = &state.monitors[state.monitor];
    let region = fitted(region, monitor);
    if state.entries.iter().enumerate().any(|(i, e)| {
        i != state.selected
            && e.monitor == monitor.device
            && e.region.is_some_and(|r| r.overlaps(region))
    }) {
        state.note =
            tr("That region is occupied. Choose free cells or clear the other category.").into();
        return;
    }
    let entry = &mut state.entries[state.selected];
    entry.monitor = monitor.device.clone();
    entry.region = Some(region);
    state.note = tr("Region selected. Icon size stays fixed; overflow scrolls.").into();
}
unsafe fn paint(hwnd: HWND, dc: HDC) {
    unsafe {
        AUX_PAINTS.fetch_add(1, Ordering::Relaxed);
        let old_font = SelectObject(
            dc,
            SendMessageW(GetDlgItem(hwnd, CATEGORY), WM_GETFONT, 0, 0) as HGDIOBJ,
        );
        SetBkMode(dc, TRANSPARENT as i32);
        let mut bounds = RECT::default();
        GetClientRect(hwnd, &mut bounds);
        let dpi = window_dpi(hwnd);
        let s = |v| scale(v, dpi);
        fill(dc, bounds, theme::color(249, 250, 253));
        theme::heading(
            dc,
            tr("Arrange categories on screen"),
            RECT {
                left: s(24),
                top: s(18),
                right: bounds.right - s(24),
                bottom: s(54),
            },
            22,
            theme::color(31, 44, 63),
            true,
            dpi,
        );
        draw(
            dc,
            tr(
                "Choose a category and monitor. Drag a rectangle; arrow keys move the selected region.",
            ),
            RECT {
                left: s(24),
                top: s(112),
                right: bounds.right - s(24),
                bottom: s(150),
            },
            theme::color(106, 119, 138),
            DT_WORDBREAK,
        );
        let grid = canvas(hwnd);
        fill(dc, grid, theme::color(228, 233, 241));
        if let Some(state) = state(hwnd) {
            for row in 0..ROWS {
                for col in 0..COLUMNS {
                    let r = RECT {
                        left: grid.left
                            + (grid.right - grid.left) * col as i32 / COLUMNS as i32
                            + s(2),
                        top: grid.top + (grid.bottom - grid.top) * row as i32 / ROWS as i32 + s(2),
                        right: grid.left
                            + (grid.right - grid.left) * (col + 1) as i32 / COLUMNS as i32
                            - s(2),
                        bottom: grid.top
                            + (grid.bottom - grid.top) * (row + 1) as i32 / ROWS as i32
                            - s(2),
                    };
                    fill(dc, r, theme::color(255, 255, 255));
                }
            }
            let region_rect = |r: Region| RECT {
                left: grid.left + (grid.right - grid.left) * r.left as i32 / COLUMNS as i32 + s(3),
                top: grid.top + (grid.bottom - grid.top) * r.top as i32 / ROWS as i32 + s(3),
                right: grid.left
                    + (grid.right - grid.left) * (r.left + r.columns) as i32 / COLUMNS as i32
                    - s(3),
                bottom: grid.top + (grid.bottom - grid.top) * (r.top + r.rows) as i32 / ROWS as i32
                    - s(3),
            };
            for (index, entry) in state.entries.iter().enumerate() {
                if entry.monitor == state.monitors[state.monitor].device
                    && let Some(region) = entry.region
                {
                    let mut r = region_rect(region);
                    fill(
                        dc,
                        r,
                        if index == state.selected {
                            theme::color(224, 231, 248)
                        } else {
                            theme::color(239, 243, 249)
                        },
                    );
                    r.left += s(10);
                    r.top += s(10);
                    r.right -= s(10);
                    draw(
                        dc,
                        &entry.name,
                        r,
                        theme::color(31, 44, 63),
                        DT_SINGLELINE | DT_END_ELLIPSIS,
                    );
                }
            }
            if let Some(region) = state.pending {
                let r = region_rect(fitted(region, &state.monitors[state.monitor]));
                let brush = CreateSolidBrush(theme::color(82, 102, 204));
                FrameRect(dc, &r, brush);
                DeleteObject(brush);
            }
            draw(
                dc,
                &state.note,
                RECT {
                    left: s(24),
                    top: bounds.bottom - s(90),
                    right: bounds.right - s(24),
                    bottom: bounds.bottom - s(58),
                },
                theme::color(106, 119, 138),
                DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
        SelectObject(dc, old_font);
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_DRAWITEM => theme::draw_control(lp as *const DRAWITEMSTRUCT),
            WM_MEASUREITEM => {
                let item = &mut *(lp as *mut MEASUREITEMSTRUCT);
                item.itemHeight = scale(26, window_dpi(hwnd)) as u32;
                1
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                let dc = wp as HDC;
                SetTextColor(dc, theme::color(31, 44, 63));
                SetBkColor(dc, theme::color(255, 255, 255));
                SetDCBrushColor(dc, theme::color(255, 255, 255));
                GetStockObject(DC_BRUSH) as isize
            }
            WM_NCCREATE => {
                let create = &*(lp as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            WM_NCDESTROY => {
                let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if !pointer.is_null() {
                    drop(Box::from_raw(pointer));
                }
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            WM_CLOSE => {
                ShowWindow(hwnd, SW_HIDE);
                0
            }
            WM_ERASEBKGND => 1,
            WM_SIZE => {
                position(hwnd);
                InvalidateRect(hwnd, null(), 0);
                0
            }
            WM_GETMINMAXINFO => {
                (*(lp as *mut MINMAXINFO)).ptMinTrackSize = POINT {
                    x: scale(760, window_dpi(hwnd)),
                    y: scale(580, window_dpi(hwnd)),
                };
                0
            }
            WM_COMMAND => {
                let id = word(wp) as i32;
                if id == APPLY {
                    PostMessageW(
                        GetWindow(hwnd, GW_OWNER),
                        WM_COMMAND,
                        ARRANGE_APPLY as usize,
                        0,
                    );
                    return 0;
                }
                if let Some(state) = state(hwnd) {
                    match id {
                        CATEGORY if word(wp >> 16) == CBN_SELCHANGE as u16 => {
                            state.selected =
                                SendMessageW(GetDlgItem(hwnd, CATEGORY), CB_GETCURSEL, 0, 0).max(0)
                                    as usize;
                            if let Some(entry) = state.entries.get(state.selected)
                                && let Some(index) = state
                                    .monitors
                                    .iter()
                                    .position(|m| m.device == entry.monitor)
                            {
                                state.monitor = index;
                                SendMessageW(GetDlgItem(hwnd, MONITOR), CB_SETCURSEL, index, 0);
                            }
                        }
                        MONITOR if word(wp >> 16) == CBN_SELCHANGE as u16 => {
                            state.monitor =
                                SendMessageW(GetDlgItem(hwnd, MONITOR), CB_GETCURSEL, 0, 0).max(0)
                                    as usize;
                        }
                        AUTO => auto_arrange(state),
                        CLEAR => {
                            if let Some(entry) = state.entries.get_mut(state.selected) {
                                entry.region = None;
                            }
                        }
                        _ => {}
                    }
                }
                InvalidateRect(hwnd, null(), 0);
                0
            }
            WM_LBUTTONDOWN => {
                if let Some(at) = cell(hwnd, lp)
                    && let Some(state) = state(hwnd)
                {
                    state.drag = Some(at);
                    state.pending = Some(Region::from_cells(at, at));
                    SetCapture(hwnd);
                    SetFocus(hwnd);
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_MOUSEMOVE => {
                if let Some(at) = cell(hwnd, lp)
                    && let Some(state) = state(hwnd)
                    && let Some(start) = state.drag
                {
                    let next = Region::from_cells(start, at);
                    if state.pending != Some(next) {
                        state.pending = Some(next);
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
                0
            }
            WM_LBUTTONUP => {
                ReleaseCapture();
                if let Some(state) = state(hwnd) {
                    if let Some(region) = state.pending.take() {
                        assign(state, region);
                    }
                    state.drag = None;
                }
                InvalidateRect(hwnd, null(), 0);
                0
            }
            WM_CAPTURECHANGED => {
                if let Some(state) = state(hwnd) {
                    state.drag = None;
                }
                0
            }
            WM_KEYDOWN => {
                if let Some(state) = state(hwnd)
                    && let Some(entry) = state.entries.get(state.selected)
                    && let Some(mut r) = entry.region
                {
                    match wp as u16 {
                        VK_LEFT => r.left = r.left.saturating_sub(1),
                        VK_RIGHT => r.left = (r.left + 1).min(COLUMNS - r.columns),
                        VK_UP => r.top = r.top.saturating_sub(1),
                        VK_DOWN => r.top = (r.top + 1).min(ROWS - r.rows),
                        _ => return DefWindowProcW(hwnd, msg, wp, lp),
                    }
                    assign(state, r);
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_PAINT | WM_PRINTCLIENT => {
                let mut ps: PAINTSTRUCT = zeroed();
                let dc = if msg == WM_PAINT {
                    BeginPaint(hwnd, &mut ps)
                } else {
                    wp as HDC
                };
                paint(hwnd, dc);
                if msg == WM_PAINT {
                    EndPaint(hwnd, &ps);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
