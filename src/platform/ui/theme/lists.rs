//! Native list semantics with compact icon rows, rather than table-like text dumps.
use super::*;
pub(super) unsafe fn draw_list(raw: *const DRAWITEMSTRUCT) -> LRESULT {
    unsafe {
        let item = &*raw;
        OWNER_DRAWS.fetch_add(1, Ordering::Relaxed);
        let dc = item.hDC;
        let dpi = window_dpi(item.hwndItem);
        fill(dc, item.rcItem, rgb(255, 255, 255));
        if item.itemID == u32::MAX {
            return 1;
        }
        let length = SendMessageW(item.hwndItem, LB_GETTEXTLEN, item.itemID as usize, 0);
        if !(0..4096).contains(&length) {
            return 1;
        }
        let mut text = vec![0u16; length as usize + 1];
        SendMessageW(
            item.hwndItem,
            LB_GETTEXT,
            item.itemID as usize,
            text.as_mut_ptr() as _,
        );
        let text = String::from_utf16_lossy(&text[..length as usize]);
        let id = item.CtlID as u16;
        let (title, detail) = if id == GROUPS {
            text.rsplit_once(" (")
                .map(|(n, c)| {
                    let count = c.trim_end_matches(')');
                    (
                        n,
                        format!("{} {}", count, if count == "1" { "item" } else { "items" }),
                    )
                })
                .unwrap_or((&text, String::new()))
        } else {
            text.split_once('·')
                .map(|(n, c)| (n.trim(), c.trim().to_owned()))
                .unwrap_or((&text, String::new()))
        };
        let group = id == GROUPS || id == PROPOSALS;
        let kind = if group {
            title
        } else {
            crate::model::category_for(title.rsplit('.').next().unwrap_or(""))
        };
        let glyph = icons::category(kind);
        let (color, tint) = match kind {
            "Images" => (rgb(186, 101, 63), rgb(255, 239, 229)),
            "Data" => (rgb(39, 136, 109), rgb(228, 246, 238)),
            "Code" => (rgb(121, 92, 184), rgb(240, 233, 255)),
            "Folders" => (rgb(168, 121, 39), rgb(255, 245, 216)),
            _ => (rgb(61, 119, 194), rgb(231, 240, 255)),
        };
        let mut row = item.rcItem;
        InflateRect(&mut row, -scale(3, dpi), -scale(2, dpi));
        if item.itemState & ODS_SELECTED != 0 {
            rounded(
                dc,
                row,
                rgb(234, 240, 253),
                rgb(216, 227, 249),
                scale(8, dpi),
            );
        }
        let badge = RECT {
            left: row.left + scale(8, dpi),
            top: row.top + scale(5, dpi),
            right: row.left + scale(40, dpi),
            bottom: row.top + scale(37, dpi),
        };
        rounded(dc, badge, tint, tint, scale(8, dpi));
        draw_icon(
            dc,
            glyph,
            badge.left + scale(6, dpi),
            badge.top + scale(6, dpi),
            scale(20, dpi),
            color,
        );
        let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0);
        let old = SelectObject(dc, font as _);
        SetBkMode(dc, TRANSPARENT as _);
        let text_left = badge.right + scale(10, dpi);
        draw(
            dc,
            title,
            RECT {
                left: text_left,
                top: row.top + scale(5, dpi),
                right: row.right - scale(7, dpi),
                bottom: row.top + scale(23, dpi),
            },
            INK,
            DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        let small = CreateFontW(
            -scale(12, dpi),
            0,
            0,
            0,
            FW_NORMAL as _,
            0,
            0,
            0,
            DEFAULT_CHARSET as _,
            0,
            0,
            CLEARTYPE_QUALITY as _,
            DEFAULT_PITCH as _,
            w!("Segoe UI"),
        );
        SelectObject(dc, small);
        draw(
            dc,
            &detail,
            RECT {
                left: text_left,
                top: row.top + scale(25, dpi),
                right: row.right - scale(7, dpi),
                bottom: row.bottom,
            },
            MUTED,
            DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(dc, old);
        DeleteObject(small);
        if item.itemState & ODS_FOCUS != 0 {
            DrawFocusRect(dc, &row);
        }
        1
    }
}
