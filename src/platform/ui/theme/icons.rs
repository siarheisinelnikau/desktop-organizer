//! Original GDI line icons. No font glyphs, downloaded artwork or raster dependencies.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::platform::ui) enum Glyph {
    Grid,
    Sparkle,
    Gear,
    Folder,
    File,
    Image,
    Table,
    Code,
    Archive,
    Refresh,
    Plus,
    Pencil,
    Remove,
    Arrow,
    Eye,
    Download,
    Shield,
    Power,
}
pub(super) fn category(name: &str) -> Glyph {
    match name {
        "Images" => Glyph::Image,
        "Data" => Glyph::Table,
        "Code" => Glyph::Code,
        "Folders" => Glyph::Folder,
        "Archives" => Glyph::Archive,
        "Applications" => Glyph::Grid,
        _ => Glyph::File,
    }
}
pub(in crate::platform::ui) unsafe fn draw_icon(
    dc: HDC,
    glyph: Glyph,
    x: i32,
    y: i32,
    size: i32,
    color: u32,
) {
    unsafe {
        SaveDC(dc);
        let p = |v: i32| v * size / 24;
        let pen = CreatePen(PS_SOLID, (size / 12).max(1), color);
        SelectObject(dc, pen);
        SelectObject(dc, GetStockObject(NULL_BRUSH));
        let line = |a: i32, b: i32, c: i32, d: i32| {
            MoveToEx(dc, x + p(a), y + p(b), null_mut());
            LineTo(dc, x + p(c), y + p(d));
        };
        let rect = |a, b, c, d| {
            RoundRect(dc, x + p(a), y + p(b), x + p(c), y + p(d), p(3), p(3));
        };
        let diamond = |cx: i32, cy: i32, r: i32| {
            line(cx, cy - r, cx + r, cy);
            line(cx + r, cy, cx, cy + r);
            line(cx, cy + r, cx - r, cy);
            line(cx - r, cy, cx, cy - r);
        };
        match glyph {
            Glyph::Grid => {
                for a in [3, 14] {
                    for b in [3, 14] {
                        rect(a, b, a + 7, b + 7);
                    }
                }
            }
            Glyph::Sparkle => {
                diamond(10, 13, 7);
                diamond(19, 5, 3);
                line(19, 16, 19, 22);
                line(16, 19, 22, 19);
            }
            Glyph::Gear => {
                Ellipse(dc, x + p(5), y + p(5), x + p(19), y + p(19));
                Ellipse(dc, x + p(9), y + p(9), x + p(15), y + p(15));
                for (a, b, c, d) in [
                    (12, 1, 12, 5),
                    (12, 19, 12, 23),
                    (1, 12, 5, 12),
                    (19, 12, 23, 12),
                    (4, 4, 7, 7),
                    (17, 17, 20, 20),
                    (4, 20, 7, 17),
                    (17, 7, 20, 4),
                ] {
                    line(a, b, c, d);
                }
            }
            Glyph::Folder => {
                line(3, 8, 3, 4);
                line(3, 4, 10, 4);
                line(10, 4, 13, 8);
                rect(2, 8, 22, 20);
            }
            Glyph::File => {
                line(5, 3, 15, 3);
                line(15, 3, 20, 8);
                line(20, 8, 20, 21);
                line(20, 21, 5, 21);
                line(5, 21, 5, 3);
                line(15, 3, 15, 8);
                line(15, 8, 20, 8);
                line(8, 12, 16, 12);
                line(8, 16, 16, 16);
            }
            Glyph::Image => {
                rect(2, 3, 22, 21);
                Ellipse(dc, x + p(6), y + p(6), x + p(10), y + p(10));
                line(3, 19, 10, 12);
                line(10, 12, 14, 16);
                line(14, 16, 18, 11);
                line(18, 11, 22, 15);
            }
            Glyph::Table => {
                rect(3, 3, 21, 21);
                line(3, 9, 21, 9);
                line(3, 15, 21, 15);
                line(10, 3, 10, 21);
            }
            Glyph::Code => {
                line(8, 6, 2, 12);
                line(2, 12, 8, 18);
                line(16, 6, 22, 12);
                line(22, 12, 16, 18);
                line(14, 3, 10, 21);
            }
            Glyph::Archive => {
                rect(3, 6, 21, 21);
                rect(2, 2, 22, 6);
                line(9, 11, 15, 11);
            }
            Glyph::Refresh => {
                Arc(
                    dc,
                    x + p(3),
                    y + p(3),
                    x + p(21),
                    y + p(21),
                    x + p(19),
                    y + p(5),
                    x + p(20),
                    y + p(18),
                );
                line(19, 2, 19, 8);
                line(19, 8, 13, 8);
            }
            Glyph::Plus => {
                line(12, 4, 12, 20);
                line(4, 12, 20, 12);
            }
            Glyph::Pencil => {
                line(4, 17, 17, 4);
                line(17, 4, 21, 8);
                line(21, 8, 8, 21);
                line(8, 21, 3, 22);
                line(3, 22, 4, 17);
                line(14, 7, 18, 11);
            }
            Glyph::Remove => {
                Ellipse(dc, x + p(3), y + p(3), x + p(21), y + p(21));
                line(7, 12, 17, 12);
            }
            Glyph::Arrow => {
                line(3, 12, 21, 12);
                line(15, 6, 21, 12);
                line(21, 12, 15, 18);
            }
            Glyph::Eye => {
                Ellipse(dc, x + p(2), y + p(6), x + p(22), y + p(18));
                Ellipse(dc, x + p(9), y + p(9), x + p(15), y + p(15));
            }
            Glyph::Download => {
                line(12, 2, 12, 16);
                line(6, 10, 12, 16);
                line(12, 16, 18, 10);
                line(3, 16, 3, 22);
                line(3, 22, 21, 22);
                line(21, 22, 21, 16);
            }
            Glyph::Shield => {
                line(12, 2, 21, 6);
                line(21, 6, 19, 16);
                line(19, 16, 12, 22);
                line(12, 22, 5, 16);
                line(5, 16, 3, 6);
                line(3, 6, 12, 2);
                line(7, 12, 11, 16);
                line(11, 16, 17, 9);
            }
            Glyph::Power => {
                Arc(
                    dc,
                    x + p(3),
                    y + p(3),
                    x + p(21),
                    y + p(21),
                    x + p(8),
                    y + p(4),
                    x + p(16),
                    y + p(4),
                );
                line(12, 1, 12, 11);
            }
        }
        RestoreDC(dc, -1);
        DeleteObject(pen);
    }
}
