//! A restrained, local-first desktop interface. Native controls retain keyboard/UIA support.
use super::*;

pub(super) const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
const NAV: u32 = rgb(24, 34, 49);
const BG: u32 = rgb(245, 247, 251);
const INK: u32 = rgb(31, 44, 63);
const MUTED: u32 = rgb(106, 119, 138);
const BLUE: u32 = rgb(53, 105, 224);
pub(super) unsafe fn application_icon(instance: HINSTANCE) -> Result<HICON> {
    unsafe {
        let mut pixels = vec![0u8; 32 * 32 * 4];
        let mask = [0u8; 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                let tile = ((6..14).contains(&x) || (18..26).contains(&x))
                    && ((6..14).contains(&y) || (18..26).contains(&y));
                let color = if tile {
                    [255, 255, 255, 255]
                } else {
                    [224, 105, 53, 255]
                };
                pixels[(y * 32 + x) * 4..(y * 32 + x + 1) * 4].copy_from_slice(&color);
            }
        }
        let icon = CreateIcon(instance, 32, 32, 1, 32, mask.as_ptr(), pixels.as_ptr());
        ensure!(!icon.is_null(), "Cannot create application icon");
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
            SHOW_GROUPS | SCAN | OFFLINE | APPLY_VISUAL | SAVE_SETTINGS
        );
        let pressed = item.itemState & ODS_SELECTED != 0;
        let background = if nav {
            if selected || pressed {
                rgb(47, 65, 91)
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
        let color = if nav || primary {
            rgb(255, 255, 255)
        } else {
            INK
        };
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
        let mut rect = item.rcItem;
        rect.left += 12;
        rect.right -= 12;
        DrawTextW(
            dc,
            text.as_ptr(),
            n,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | if nav { DT_LEFT } else { DT_CENTER },
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
            for control in self.controls.iter().filter(|c| c.id == 0) {
                DestroyWindow(control.hwnd);
            }
            self.controls.retain(|c| c.id != 0);
            for c in &mut self.controls {
                c.rect = match c.id {
                    PAGE_LIBRARY => (16, 122, 160, 42),
                    PAGE_AI => (16, 174, 160, 42),
                    PAGE_SETTINGS => (16, 226, 160, 42),
                    SHOW_GROUPS => (806, 26, 174, 38),
                    ROOT => (240, 150, 466, 34),
                    BROWSE => (718, 150, 106, 34),
                    SCAN => (836, 150, 124, 34),
                    GROUPS => (240, 288, 180, 190),
                    GROUP_NAME => (240, 494, 180, 32),
                    RENAME_GROUP => (240, 538, 84, 34),
                    NEW_GROUP => (334, 538, 86, 34),
                    DELETE_GROUP => (240, 586, 180, 34),
                    ITEMS => (482, 288, 478, 218),
                    ASSIGN => (482, 524, 238, 36),
                    REMOVE => (732, 524, 228, 36),
                    AI_ITEMS => (240, 142, 310, 272),
                    SELECT_ALL => (240, 426, 140, 34),
                    OFFLINE => (390, 426, 160, 34),
                    JSON => {
                        c.page = 9;
                        (0, 0, 0, 0)
                    }
                    APPLY_VISUAL => (590, 426, 370, 34),
                    CONSENT => (240, 480, 520, 30),
                    REQUEST_AI => (780, 476, 180, 36),
                    MOVE_OPT => (240, 531, 720, 28),
                    PREVIEW_MOVES => (240, 571, 186, 34),
                    APPLY_MOVES => (438, 571, 260, 34),
                    UNDO => (710, 571, 250, 34),
                    MOVES => (240, 618, 720, 46),
                    FULLSCREEN => (240, 150, 476, 28),
                    OPACITY => (552, 187, 70, 30),
                    ACCENT => (738, 174, 222, 34),
                    BASE => (342, 293, 618, 32),
                    MODEL => (342, 335, 618, 32),
                    KEY => (342, 377, 350, 32),
                    CLEAR_KEY => (710, 377, 250, 30),
                    SAVE_SETTINGS => (774, 463, 186, 36),
                    AUTO_CHECK => (240, 557, 328, 28),
                    CHECK => (590, 551, 174, 34),
                    INSTALL => (776, 551, 184, 34),
                    UPDATE_STATUS => (240, 600, 720, 54),
                    STATUS => (240, 702, 530, 42),
                    QUIT => (16, 714, 160, 34),
                    _ => c.rect,
                };
                if !matches!(
                    c.id,
                    ROOT | GROUPS
                        | GROUP_NAME
                        | ITEMS
                        | AI_ITEMS
                        | JSON
                        | MOVES
                        | BASE
                        | MODEL
                        | KEY
                        | OPACITY
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
                (PAGE_LIBRARY, "Your library"),
                (PAGE_AI, "Organize"),
                (PAGE_SETTINGS, "Settings"),
                (SHOW_GROUPS, "Show desktop groups"),
                (SCAN, "Add files"),
                (NEW_GROUP, "+ New"),
                (ASSIGN, "Move to selected group"),
                (REMOVE, "Remove from library"),
                (SELECT_ALL, "Select all"),
                (OFFLINE, "Group by type"),
                (APPLY_VISUAL, "Apply groups"),
                (REQUEST_AI, "Ask my AI provider"),
                (PREVIEW_MOVES, "Preview file moves"),
                (APPLY_MOVES, "Move previewed files"),
                (UNDO, "Undo file moves"),
                (CHECK, "Check for updates"),
                (INSTALL, "Download & install"),
                (QUIT, "Quit"),
                (DELETE_GROUP, "Remove group"),
            ] {
                self.set(id, text);
            }
            self.set(
                CONSENT,
                "Allow sharing selected names and file types with my AI provider",
            );
            self.set(
                MOVE_OPT,
                "Also organize files into folders — optional, with a separate preview",
            );
            self.set(FULLSCREEN, "Hide groups while an app is in fullscreen");
            self.set(MOVES,"Your files stay in place. Folder moves require a separate preview and confirmation.");
            self.label(1, "Add a folder", (240, 119, 450, 24));
            self.label(
                1,
                "Files stay where they are. You can also drop files onto a desktop group.",
                (240, 192, 720, 22),
            );
            self.label(1, "Your groups", (240, 252, 180, 24));
            self.label(1, "Files in your library", (482, 252, 478, 24));
            self.label(1,"Select files, then choose a group on the left.\r\nDrag tiles between desktop groups, or double-click a tile to open it.\r\nRemoving an item here never deletes the original file.",(482,580,478,64));
            self.label(2, "Choose files", (240, 111, 310, 24));
            self.label(2, "Group preview", (590, 111, 370, 24));
            self.control(
                PROPOSALS,
                2,
                w!("LISTBOX"),
                "",
                LBS_NOTIFY as u32 | WS_VSCROLL,
                (590, 142, 370, 152),
            );
            self.edit(PROPOSAL_NAME, 2, "", (590, 308, 240, 32), 0);
            self.button(RENAME_PROPOSAL, 2, "Rename", (842, 308, 118, 32));
            self.button(
                ASSIGN_PROPOSAL,
                2,
                "Move selected files into this group",
                (590, 352, 370, 34),
            );
            self.label(
                2,
                "Nothing changes until you apply your groups.",
                (590, 394, 370, 22),
            );
            for id in [RENAME_PROPOSAL, ASSIGN_PROPOSAL] {
                let h = self.c(id);
                SetWindowLongPtrW(
                    h,
                    GWL_STYLE,
                    (WS_CHILD | WS_TABSTOP | BS_OWNERDRAW as u32) as _,
                );
            }
            self.label(3, "Appearance", (240, 115, 600, 26));
            self.label(3, "Opacity (150–255)", (240, 191, 290, 24));
            self.label(3, "AI provider", (240, 254, 600, 26));
            self.label(3, "Base URL", (240, 298, 96, 24));
            self.label(3, "Model", (240, 340, 96, 24));
            self.label(3, "New token", (240, 382, 96, 24));
            self.label(3,"Optional. Your saved token stays encrypted on this PC; leave the field blank to keep it.",(240,422,720,26));
            self.label(
                3,
                &format!("Updates  ·  Version {}", updates::INSTALLED),
                (240, 518, 700, 24),
            );
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
    pub(super) unsafe fn paint_main(&self) {
        unsafe {
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(self.hwnd, &mut ps);
            if IsWindowVisible(self.hwnd) != 0 && IsIconic(self.hwnd) == 0 {
                self.render_main(dc);
            }
            EndPaint(self.hwnd, &ps);
        }
    }
    pub(super) unsafe fn render_main(&self, dc: HDC) {
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
            rounded(dc, rect(20, 26, 36, 36), BLUE, BLUE, 10);
            for x in [28, 42] {
                for y in [34, 48] {
                    fill(dc, rect(x, y, 8, 8), rgb(255, 255, 255));
                }
            }
            heading(
                dc,
                "DesktopOrganizer",
                rect(20, 76, 170, 32),
                16,
                rgb(255, 255, 255),
                true,
                dpi,
            );
            heading(
                dc,
                "LOCAL BY DEFAULT",
                rect(20, 660, 166, 24),
                11,
                rgb(147, 164, 189),
                false,
                dpi,
            );
            heading(
                dc,
                &format!("Version {}", updates::INSTALLED),
                rect(20, 686, 166, 24),
                12,
                rgb(147, 164, 189),
                false,
                dpi,
            );
            let (title, subtitle) = match self.page {
                1 => (
                    "Your library",
                    "Everyday files, with a little more breathing room.",
                ),
                2 => (
                    "A home for every file",
                    "Group locally by type, or get suggestions from your own AI provider.",
                ),
                _ => (
                    "Make it yours",
                    "A few thoughtful settings for your desktop and your workflow.",
                ),
            };
            heading(dc, title, rect(220, 23, 560, 40), 28, INK, true, dpi);
            heading(dc, subtitle, rect(220, 70, 760, 24), 14, MUTED, false, dpi);
            let cards: &[(i32, i32, i32, i32)] = match self.page {
                1 => &[
                    (220, 104, 760, 124),
                    (220, 240, 220, 430),
                    (462, 240, 518, 430),
                ],
                2 => &[
                    (220, 104, 350, 360),
                    (574, 104, 406, 360),
                    (220, 470, 760, 200),
                ],
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
            if self.page == 1 && self.layout.items.is_empty() {
                // The empty list is intentionally hidden below, so this is the actual empty-state surface.
                heading(
                    dc,
                    "A calmer desktop starts here",
                    rect(502, 340, 438, 32),
                    19,
                    INK,
                    true,
                    dpi,
                );
                heading(
                    dc,
                    "Add a folder above, or drop files onto your Inbox group. Your originals always stay in place.",
                    rect(502, 388, 398, 70),
                    15,
                    MUTED,
                    false,
                    dpi,
                );
            }
            for control in self.controls.iter().filter(|c| {
                c.page == self.page
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
