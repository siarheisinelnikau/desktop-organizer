//! A restrained, local-first desktop interface. Native controls retain keyboard/UIA support.
use super::*;
use windows_sys::Win32::System::SystemServices::SS_ENDELLIPSIS;
mod icons;
mod lists;
pub(super) use icons::{Glyph, draw_icon};
pub(super) unsafe fn draw_control(item: *const DRAWITEMSTRUCT) -> LRESULT {
    unsafe {
        if item.is_null() {
            return 0;
        }
        if (*item).CtlType == ODT_LISTBOX {
            lists::draw_list(item)
        } else {
            draw_button(item)
        }
    }
}

pub(super) const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
const NAV: u32 = rgb(239, 243, 249);
const BG: u32 = rgb(249, 250, 253);
const INK: u32 = rgb(31, 44, 63);
const MUTED: u32 = rgb(106, 119, 138);
const BLUE: u32 = rgb(82, 102, 204);
pub(super) unsafe fn application_icon(instance: HINSTANCE, small: bool) -> Result<HICON> {
    unsafe {
        let mut icon = null_mut();
        let metric = if small { LIM_SMALL } else { LIM_LARGE };
        ensure!(
            LoadIconMetric(instance, 1usize as PCWSTR, metric, &mut icon) >= 0 && !icon.is_null(),
            "Cannot load the embedded application icon"
        );
        Ok(icon)
    }
}

pub(super) unsafe fn draw_button(item: *const DRAWITEMSTRUCT) -> LRESULT {
    unsafe {
        if item.is_null() || (*item).CtlType != ODT_BUTTON {
            return 0;
        }
        let item = &*item;
        OWNER_DRAWS.fetch_add(1, Ordering::Relaxed);
        let dc = item.hDC;
        let id = item.CtlID as u16;
        let nav = matches!(id, PAGE_LIBRARY | PAGE_AI | PAGE_SETTINGS | QUIT);
        let selected = GetWindowLongPtrW(item.hwndItem, GWLP_USERDATA) != 0;
        let primary = matches!(
            id,
            BROWSE | ADD_DESKTOP | PREVIEW_MOVES | APPLY_MOVES | SAVE_SETTINGS
        );
        let pressed = item.itemState & ODS_SELECTED != 0;
        let background = if nav {
            if selected || pressed {
                rgb(224, 231, 248)
            } else {
                NAV
            }
        } else if primary {
            if pressed { rgb(35, 79, 183) } else { BLUE }
        } else if pressed {
            rgb(232, 237, 246)
        } else {
            rgb(255, 255, 255)
        };
        let color = if primary { rgb(255, 255, 255) } else { INK };
        let disabled = item.itemState & ODS_DISABLED != 0;
        fill(dc, item.rcItem, if nav { NAV } else { rgb(255, 255, 255) });
        rounded(
            dc,
            item.rcItem,
            if disabled {
                rgb(240, 243, 248)
            } else {
                background
            },
            if nav || primary {
                background
            } else {
                rgb(219, 225, 235)
            },
            8,
        );
        let mut text = [0u16; 256];
        let n = GetWindowTextW(item.hwndItem, text.as_mut_ptr(), text.len() as _);
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0);
        let old = SelectObject(dc, font as _);
        SetBkMode(dc, TRANSPARENT as _);
        SetTextColor(dc, if disabled { MUTED } else { color });
        let glyph = match id {
            PAGE_LIBRARY => Some(Glyph::Grid),
            PAGE_AI | OFFLINE | REQUEST_AI | AI_SETUP => Some(Glyph::Sparkle),
            PAGE_SETTINGS => Some(Glyph::Gear),
            BROWSE | ADD_DESKTOP | SCAN_DESKTOP | TIDY_ADD_FOLDER => Some(Glyph::Folder),
            SCAN => Some(Glyph::Refresh),
            NEW_GROUP => Some(Glyph::Plus),
            RENAME_GROUP | RENAME_PROPOSAL => Some(Glyph::Pencil),
            REMOVE | DELETE_GROUP => Some(Glyph::Remove),
            ASSIGN | ASSIGN_PROPOSAL | APPLY_VISUAL | PREVIEW_MOVES | BACK_TO_PLAN => {
                Some(Glyph::Arrow)
            }
            SHOW_GROUPS => Some(Glyph::Eye),
            CHECK | TEST_AI => Some(Glyph::Refresh),
            INSTALL => Some(Glyph::Download),
            SAVE_SETTINGS => Some(Glyph::Shield),
            QUIT => Some(Glyph::Power),
            _ => None,
        };
        let dpi = window_dpi(item.hwndItem);
        let icon_size = scale(18, dpi);
        let mut rect = item.rcItem;
        if let Some(glyph) = glyph {
            let x = rect.left + scale(12, dpi);
            let y = rect.top + (rect.bottom - rect.top - icon_size) / 2;
            draw_icon(
                dc,
                glyph,
                x,
                y,
                icon_size,
                if disabled {
                    MUTED
                } else if nav {
                    if selected { BLUE } else { MUTED }
                } else {
                    color
                },
            );
            rect.left += scale(39, dpi);
            rect.right -= scale(9, dpi);
        } else {
            rect.left += scale(10, dpi);
            rect.right -= scale(10, dpi);
        }
        SetTextColor(dc, if disabled { MUTED } else { color });
        DrawTextW(
            dc,
            text.as_ptr(),
            n,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS | DT_LEFT,
        );
        if item.itemState & ODS_FOCUS != 0 {
            let mut focus = item.rcItem;
            InflateRect(&mut focus, -4, -4);
            DrawFocusRect(dc, &focus);
        }
        SelectObject(dc, old);
        1
    }
}

pub(super) unsafe fn rounded(dc: HDC, rect: RECT, color: u32, border: u32, radius: i32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}
pub(super) unsafe fn heading(
    dc: HDC,
    text: &str,
    rect: RECT,
    size: i32,
    color: u32,
    bold: bool,
    dpi: u32,
) {
    unsafe {
        let font = CreateFontW(
            -scale(size, dpi),
            0,
            0,
            0,
            if bold { FW_SEMIBOLD } else { FW_NORMAL } as _,
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
        let old = SelectObject(dc, font);
        draw(dc, text, rect, color, DT_WORDBREAK);
        SelectObject(dc, old);
        DeleteObject(font);
    }
}

impl App {
    pub(super) unsafe fn redesign_controls(&mut self) {
        unsafe {
            for c in self.controls.iter().filter(|c| c.id == 0) {
                DestroyWindow(c.hwnd);
            }
            self.controls.retain(|c| c.id != 0);
            for c in &mut self.controls {
                c.rect = match c.id {
                    PAGE_LIBRARY => (16, 26, 160, 44),
                    PAGE_AI => (16, 80, 160, 44),
                    PAGE_SETTINGS => (16, 134, 160, 44),
                    SHOW_GROUPS => (806, 26, 174, 40),
                    ROOT | JSON | OPACITY => {
                        c.page = 9;
                        (0, 0, 0, 0)
                    }
                    BROWSE => (780, 128, 180, 40),
                    SCAN => (654, 128, 114, 40),
                    GROUPS => (238, 286, 188, 236),
                    GROUP_NAME => (240, 536, 180, 32),
                    RENAME_GROUP => (240, 580, 180, 34),
                    NEW_GROUP => (240, 236, 180, 36),
                    DELETE_GROUP => (240, 626, 180, 34),
                    ITEMS => (480, 270, 480, 324),
                    ASSIGN => (482, 612, 230, 36),
                    REMOVE => (724, 612, 236, 36),
                    AI_ITEMS => (240, 150, 310, 264),
                    SELECT_ALL => (240, 426, 140, 36),
                    OFFLINE => (390, 426, 160, 36),
                    APPLY_VISUAL => (240, 588, 310, 36),
                    CONSENT => (240, 482, 520, 28),
                    REQUEST_AI => (780, 480, 180, 36),
                    MOVE_OPT => {
                        c.page = 9;
                        (0, 0, 0, 0)
                    }
                    PREVIEW_MOVES => (590, 426, 370, 36),
                    APPLY_MOVES => {
                        c.page = 5;
                        (700, 612, 260, 40)
                    }
                    UNDO => (240, 634, 310, 34),
                    MOVES => {
                        c.page = 5;
                        (240, 150, 720, 440)
                    }
                    FULLSCREEN => (240, 156, 460, 28),
                    ACCENT => (738, 174, 222, 36),
                    BASE => {
                        c.page = 4;
                        (342, 293, 618, 32)
                    }
                    MODEL => {
                        c.page = 4;
                        (342, 350, 618, 32)
                    }
                    KEY => {
                        c.page = 4;
                        (342, 392, 350, 32)
                    }
                    CLEAR_KEY => {
                        c.page = 4;
                        (710, 392, 250, 30)
                    }
                    SAVE_SETTINGS => (774, 454, 186, 36),
                    AUTO_CHECK => (240, 556, 328, 28),
                    CHECK => (590, 550, 174, 36),
                    INSTALL => (776, 550, 184, 36),
                    UPDATE_STATUS => (240, 602, 720, 50),
                    STATUS => (240, 702, 720, 34),
                    QUIT => (16, 714, 160, 34),
                    _ => c.rect,
                };
                if !matches!(
                    c.id,
                    ROOT | JSON
                        | OPACITY
                        | GROUPS
                        | ITEMS
                        | AI_ITEMS
                        | PROPOSALS
                        | GROUP_NAME
                        | MOVES
                        | BASE
                        | MODEL
                        | KEY
                        | CONSENT
                        | MOVE_OPT
                        | FULLSCREEN
                        | AUTO_CHECK
                        | CLEAR_KEY
                        | UPDATE_STATUS
                        | STATUS
                ) {
                    let style = GetWindowLongPtrW(c.hwnd, GWL_STYLE) as u32;
                    SetWindowLongPtrW(
                        c.hwnd,
                        GWL_STYLE,
                        ((style & !(BS_TYPEMASK as u32)) | BS_OWNERDRAW as u32) as _,
                    );
                }
            }
            for (id, text) in [
                (PAGE_LIBRARY, "My groups"),
                (PAGE_AI, "Organize"),
                (PAGE_SETTINGS, "Settings"),
                (SHOW_GROUPS, "Show groups"),
                (BROWSE, "Add a folder"),
                (SCAN, "Refresh"),
                (NEW_GROUP, "New group"),
                (RENAME_GROUP, "Rename"),
                (DELETE_GROUP, "Remove"),
                (ASSIGN, "Add to group"),
                (REMOVE, "Remove from list"),
                (SELECT_ALL, "Select all"),
                (OFFLINE, "Suggest folders"),
                (APPLY_VISUAL, "Use visual groups only"),
                (REQUEST_AI, "Ask my AI"),
                (PREVIEW_MOVES, "Review file moves"),
                (APPLY_MOVES, "Move files"),
                (UNDO, "Undo moves"),
                (CHECK, "Check updates"),
                (INSTALL, "Download & install"),
                (SAVE_SETTINGS, "Save settings"),
                (QUIT, "Quit"),
            ] {
                self.set(id, text);
            }
            self.set(CONSENT, "Share selected names and file types with my AI");
            self.set(MOVE_OPT, "Move originals into folders — review first");
            self.set(FULLSCREEN, "Hide groups while I use a fullscreen app");
            self.control(
                FOLDER_NAME,
                1,
                w!("STATIC"),
                "Choose your first folder",
                0,
                (300, 128, 344, 26),
            );
            self.sync_folder_name();
            self.label(1, "Files stay right where they are.", (300, 160, 340, 22));
            self.label(1, "My files", (482, 234, 478, 26));
            self.label(2, "Files and shortcuts", (240, 116, 310, 24));
            self.label(2, "Suggested folders", (590, 116, 370, 24));
            self.control(
                PROPOSALS,
                2,
                w!("LISTBOX"),
                "",
                LBS_NOTIFY as u32 | WS_VSCROLL,
                (590, 150, 370, 144),
            );
            self.edit(PROPOSAL_NAME, 2, "", (590, 308, 228, 32), 0);
            self.button(RENAME_PROPOSAL, 2, "Rename", (830, 308, 130, 32));
            self.button(
                ASSIGN_PROPOSAL,
                2,
                "Add selected files here",
                (590, 352, 370, 36),
            );
            self.button(ADD_DESKTOP, 2, "Scan my Desktop", (240, 350, 310, 40));
            self.button(
                TIDY_ADD_FOLDER,
                2,
                "Choose another folder",
                (240, 404, 310, 36),
            );
            self.button(SCAN_DESKTOP, 2, "Scan Desktop", (806, 26, 174, 40));
            self.button(BACK_TO_PLAN, 5, "Back to folders", (240, 612, 200, 40));
            self.label(5, "Original path → destination", (240, 116, 720, 24));
            for id in [ADD_DESKTOP, TIDY_ADD_FOLDER, SCAN_DESKTOP, BACK_TO_PLAN] {
                SetWindowLongPtrW(
                    self.c(id),
                    GWL_STYLE,
                    (WS_CHILD | WS_TABSTOP | BS_OWNERDRAW as u32) as _,
                );
            }
            self.control(
                AI_TIDY_CONNECTION,
                2,
                w!("STATIC"),
                "AI not tested · check it in Settings",
                SS_ENDELLIPSIS,
                (240, 514, 520, 24),
            );
            self.label(3, "Look and feel", (240, 116, 600, 26));
            self.label(3, "Group opacity", (240, 192, 290, 24));
            self.control(
                OPACITY_SLIDER,
                3,
                w!("msctls_trackbar32"),
                "",
                TBS_NOTICKS,
                (420, 184, 230, 32),
            );
            SendMessageW(
                self.c(OPACITY_SLIDER),
                TBM_SETRANGE,
                0,
                ((100u32 << 16) | 60) as isize,
            );
            SendMessageW(
                self.c(OPACITY_SLIDER),
                TBM_SETPOS,
                1,
                ((self.layout.settings.opacity as u32 * 100 + 127) / 255).max(60) as isize,
            );
            self.control(
                OPACITY_VALUE,
                3,
                w!("STATIC"),
                &format!(
                    "{}%",
                    ((self.layout.settings.opacity as u32 * 100 + 127) / 255).max(60)
                ),
                0,
                (660, 192, 60, 24),
            );
            self.button(AI_SETUP, 3, "AI settings", (772, 326, 188, 38));
            self.button(TEST_AI, 3, "Save & test AI", (558, 454, 206, 36));
            SetWindowLongPtrW(
                self.c(TEST_AI),
                GWL_STYLE,
                (WS_CHILD | WS_TABSTOP | BS_OWNERDRAW as u32) as _,
            );
            self.control(
                AI_CONNECTION,
                3,
                w!("STATIC"),
                "AI not tested",
                SS_ENDELLIPSIS,
                (240, 464, 310, 24),
            );
            self.label(3, "AI suggestions", (240, 254, 500, 26));
            self.label(4, "Base URL", (240, 298, 96, 24));
            self.control(
                AI_TRANSPORT_NOTE,
                4,
                w!("STATIC"),
                "",
                0,
                (342, 327, 618, 20),
            );
            self.sync_ai_transport_note();
            self.label(4, "Model", (240, 355, 96, 24));
            self.label(4, "New token", (240, 397, 96, 24));
            self.label(
                4,
                "Leave the token blank to keep the saved one.",
                (240, 428, 720, 24),
            );
            self.label(
                3,
                &format!("Updates · v{}", updates::INSTALLED),
                (240, 518, 700, 24),
            );
            for id in [RENAME_PROPOSAL, ASSIGN_PROPOSAL, AI_SETUP] {
                let h = self.c(id);
                SetWindowLongPtrW(
                    h,
                    GWL_STYLE,
                    (WS_CHILD | WS_TABSTOP | BS_OWNERDRAW as u32) as _,
                );
            }
        }
    }
    pub(super) unsafe fn set_suggestions(
        &mut self,
        groups: Vec<grouping::Suggestion>,
    ) -> Result<()> {
        unsafe {
            self.set(
                JSON,
                &serde_json::to_string(&serde_json::json!({"groups":groups}))?,
            );
            let list = self.c(PROPOSALS);
            SendMessageW(list, LB_RESETCONTENT, 0, 0);
            for group in &groups {
                SendMessageW(
                    list,
                    LB_ADDSTRING,
                    0,
                    wide(format!(
                        "{}   ·   {} {}",
                        group.name,
                        group.item_ids.len(),
                        if group.item_ids.len() == 1 {
                            "file"
                        } else {
                            "files"
                        }
                    ))
                    .as_ptr() as _,
                );
            }
            SendMessageW(list, LB_SETCURSEL, 0, 0);
            self.proposal_selection();
            self.update_actions();
            InvalidateRect(self.hwnd, null(), 0);
            Ok(())
        }
    }
    pub(super) unsafe fn proposal_selection(&self) {
        unsafe {
            let row = SendMessageW(self.c(PROPOSALS), LB_GETCURSEL, 0, 0);
            if let Ok(groups) = ai::parse_content(&self.text(JSON), &self.scope)
                && row >= 0
                && let Some(group) = groups.get(row as usize)
            {
                self.set(PROPOSAL_NAME, &group.name);
            }
        }
    }
    pub(super) unsafe fn paint_main(&mut self) {
        unsafe {
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(self.hwnd, &mut ps);
            if IsWindowVisible(self.hwnd) != 0 && IsIconic(self.hwnd) == 0 {
                self.render_main(dc);
            }
            EndPaint(self.hwnd, &ps);
        }
    }
    pub(super) unsafe fn render_main(&mut self, output: HDC) {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.hwnd, &mut rect);
            let key = MainSceneKey {
                page: self.page,
                dpi: window_dpi(self.hwnd),
                width: rect.right.max(1),
                height: rect.bottom.max(1),
                scroll_x: self.main_scroll.x,
                scroll_y: self.main_scroll.y,
                state: (self.layout.items.is_empty() as u8)
                    | ((self.tidy_items.is_empty() as u8) << 1)
                    | ((self.ai_setup_open as u8) << 2)
                    | (((GetWindowTextLengthW(self.c(JSON)) > 0) as u8) << 3)
                    | (((GetWindowTextLengthW(self.c(STATUS)) > 0) as u8) << 4),
            };
            if self
                .main_surface
                .as_ref()
                .is_none_or(|surface| surface.key != key)
            {
                let reusable = self.main_surface.take().filter(|surface| {
                    surface.key.width == key.width && surface.key.height == key.height
                });
                let mut surface = if let Some(surface) = reusable {
                    surface
                } else {
                    let dc = CreateCompatibleDC(output);
                    let bitmap = CreateCompatibleBitmap(output, key.width, key.height);
                    if dc.is_null() || bitmap.is_null() {
                        if !dc.is_null() {
                            DeleteDC(dc);
                        }
                        if !bitmap.is_null() {
                            DeleteObject(bitmap);
                        }
                        self.render_main_scene(output);
                        return;
                    }
                    let previous = SelectObject(dc, bitmap);
                    MainSurface {
                        dc,
                        bitmap,
                        previous,
                        key,
                    }
                };
                self.render_main_scene(surface.dc);
                surface.key = key;
                self.main_surface = Some(surface);
            }
            let surface = self.main_surface.as_ref().unwrap();
            BitBlt(
                output, 0, 0, key.width, key.height, surface.dc, 0, 0, SRCCOPY,
            );
            MAIN_BLITS.fetch_add(1, Ordering::Relaxed);
        }
    }
    unsafe fn render_main_scene(&self, dc: HDC) {
        unsafe {
            MAIN_PAINTS.fetch_add(1, Ordering::Relaxed);
            let mut client = RECT::default();
            GetClientRect(self.hwnd, &mut client);
            let dpi = window_dpi(self.hwnd).max(96);
            fill(dc, client, BG);
            SetBkMode(dc, TRANSPARENT as _);
            let rect = |x, y, w, h| RECT {
                left: scale(x, dpi) - self.main_scroll.x,
                top: scale(y, dpi) - self.main_scroll.y,
                right: scale(x + w, dpi) - self.main_scroll.x,
                bottom: scale(y + h, dpi) - self.main_scroll.y,
            };
            fill(
                dc,
                RECT {
                    left: 0,
                    top: 0,
                    right: scale(192, dpi),
                    bottom: client.bottom,
                },
                NAV,
            );
            let (title, subtitle) = match self.page {
                1 => ("Your space", "Give everyday files a home."),
                2 => (
                    "Organize your Desktop",
                    "See a plan first. Move files only when you confirm.",
                ),
                5 => (
                    "Review file moves",
                    "Check every destination before moving the originals.",
                ),
                _ => ("Make it yours", "A few small preferences."),
            };
            heading(dc, title, rect(220, 23, 560, 40), 28, INK, true, dpi);
            heading(dc, subtitle, rect(220, 70, 760, 24), 14, MUTED, false, dpi);
            let cards: &[(i32, i32, i32, i32)] = match self.page {
                1 => &[
                    (220, 104, 760, 96),
                    (220, 220, 220, 450),
                    (462, 220, 518, 450),
                ],
                2 if self.tidy_items.is_empty() => &[(220, 104, 350, 360), (574, 104, 406, 360)],
                2 => &[
                    (220, 104, 350, 360),
                    (574, 104, 406, 360),
                    (220, 470, 760, 200),
                ],
                5 => &[(220, 104, 760, 566)],
                _ => &[
                    (220, 104, 760, 128),
                    (220, 242, 760, 212),
                    (220, 508, 760, 162),
                ],
            };
            for &(x, y, w, h) in cards {
                rounded(
                    dc,
                    rect(x, y, w, h),
                    rgb(255, 255, 255),
                    rgb(228, 233, 241),
                    14,
                );
            }
            if self.page == 2 {
                if self.tidy_items.is_empty() {
                    draw_icon(
                        dc,
                        Glyph::Folder,
                        rect(356, 166, 1, 1).left,
                        rect(356, 166, 1, 1).top,
                        scale(56, dpi),
                        BLUE,
                    );
                    heading(
                        dc,
                        "Start with your Desktop",
                        rect(240, 248, 310, 30),
                        19,
                        INK,
                        true,
                        dpi,
                    );
                    heading(
                        dc,
                        "Use the files already on your Desktop. Existing folders stay in place.",
                        rect(240, 288, 310, 54),
                        15,
                        MUTED,
                        false,
                        dpi,
                    );
                }
                if self.text(JSON).is_empty() {
                    draw_icon(
                        dc,
                        Glyph::Sparkle,
                        rect(730, 178, 1, 1).left,
                        rect(730, 178, 1, 1).top,
                        scale(44, dpi),
                        BLUE,
                    );
                    heading(
                        dc,
                        "Your folder plan starts here",
                        rect(594, 254, 346, 30),
                        19,
                        INK,
                        true,
                        dpi,
                    );
                    heading(
                        dc,
                        if self.tidy_items.is_empty() {
                            "Scan your Desktop to see proposed folders and their files. Nothing moves yet."
                        } else {
                            "Choose Suggest folders or Ask my AI to preview folders for these files."
                        },
                        rect(594, 296, 346, 74),
                        15,
                        MUTED,
                        false,
                        dpi,
                    );
                } else {
                    heading(
                        dc,
                        "Just a preview. You're in control.",
                        rect(590, 396, 370, 22),
                        14,
                        MUTED,
                        false,
                        dpi,
                    );
                }
                if !self.tidy_items.is_empty() {
                    heading(
                        dc,
                        "Prefer floating groups? This keeps all original files where they are.",
                        rect(566, 588, 394, 60),
                        15,
                        MUTED,
                        false,
                        dpi,
                    );
                }
            }
            if self.page == 1 && self.layout.items.is_empty() {
                // The empty list is intentionally hidden below, so this is the actual empty-state surface.
                heading(
                    dc,
                    "Start with a folder",
                    rect(502, 444, 438, 32),
                    19,
                    INK,
                    true,
                    dpi,
                );
                heading(
                    dc,
                    "Add your Desktop or any folder you like.
Your originals stay right where they are.",
                    rect(502, 486, 398, 64),
                    15,
                    MUTED,
                    false,
                    dpi,
                );
            }
            if self.page == 1 {
                let folder = rect(240, 122, 44, 44);
                rounded(dc, folder, rgb(255, 244, 216), rgb(255, 244, 216), 10);
                draw_icon(
                    dc,
                    Glyph::Folder,
                    folder.left + scale(8, dpi),
                    folder.top + scale(8, dpi),
                    scale(28, dpi),
                    rgb(172, 126, 39),
                );
                if self.layout.items.is_empty() {
                    draw_icon(
                        dc,
                        Glyph::Folder,
                        rect(550, 332, 1, 1).left,
                        rect(550, 332, 1, 1).top,
                        scale(74, dpi),
                        BLUE,
                    );
                }
            }
            if self.page == 3 && !self.ai_setup_open {
                let icon = rect(246, 310, 58, 58);
                rounded(dc, icon, rgb(238, 234, 253), rgb(238, 234, 253), 12);
                draw_icon(
                    dc,
                    Glyph::Sparkle,
                    icon.left + scale(12, dpi),
                    icon.top + scale(12, dpi),
                    scale(34, dpi),
                    rgb(122, 97, 190),
                );
                heading(
                    dc,
                    "A helping hand, if you want it",
                    rect(326, 304, 420, 30),
                    18,
                    INK,
                    true,
                    dpi,
                );
                heading(
                    dc,
                    "Connect your own AI for grouping ideas.",
                    rect(326, 344, 410, 46),
                    14,
                    MUTED,
                    false,
                    dpi,
                );
            }
            for control in self.controls.iter().filter(|c| {
                self.control_visible(c)
                    && matches!(
                        c.id,
                        ROOT | GROUP_NAME | PROPOSAL_NAME | BASE | MODEL | KEY | OPACITY
                    )
            }) {
                let (x, y, w, h) = control.rect;
                rounded(
                    dc,
                    rect(x - 2, y - 2, w + 4, h + 4),
                    rgb(255, 255, 255),
                    rgb(205, 214, 228),
                    8,
                );
            }
            if !self.text(STATUS).is_empty() {
                rounded(
                    dc,
                    rect(220, 686, 760, 64),
                    rgb(255, 255, 255),
                    rgb(228, 233, 241),
                    12,
                );
            }
        }
    }
}
