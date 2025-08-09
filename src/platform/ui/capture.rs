//! Finite developer-only capture of the production Win32/GDI components.
//! No tray registration, user settings, network work, activation, or desktop capture.
use super::*;
use std::{ffi::OsString, io::Write, path::Path, time::Instant};

pub(super) struct Options {
    pub diagnose: bool,
    pub output: PathBuf,
    pub screen: String,
    pub scenario: String,
    pub root: Option<PathBuf>,
    pub dpi: u32,
}
impl Options {
    pub fn parse(args: impl Iterator<Item = OsString>) -> Result<Option<Self>> {
        let args: Vec<_> = args.collect();
        if args.is_empty() {
            return Ok(None);
        }
        if !args.iter().any(|a| a == "--capture" || a == "--diagnose") {
            anyhow::bail!(
                "Unknown command line. Use --capture ABSOLUTE.png --screen library|organize|settings|group [--fixture-root PATH] [--scenario populated|empty|error|long-names|ai-setup|http-server] [--scale 100|125|150]"
            );
        }
        let mut result = Self {
            diagnose: false,
            output: PathBuf::new(),
            screen: "library".into(),
            scenario: "populated".into(),
            root: None,
            dpi: 96,
        };
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let value = args.next().context("Missing capture option value")?;
            match flag.to_str() {
                Some("--capture") => result.output = PathBuf::from(value),
                Some("--diagnose") => {
                    result.output = PathBuf::from(value);
                    result.diagnose = true;
                }
                Some("--screen") => result.screen = value.to_string_lossy().into_owned(),
                Some("--scenario") => result.scenario = value.to_string_lossy().into_owned(),
                Some("--fixture-root") => result.root = Some(value.into()),
                Some("--scale") => {
                    result.dpi = match value.to_str() {
                        Some("100") => 96,
                        Some("125") => 120,
                        Some("150") => 144,
                        _ => anyhow::bail!("Capture scale must be 100, 125 or 150"),
                    }
                }
                _ => anyhow::bail!("Unknown capture option"),
            }
        }
        ensure!(
            result.output.is_absolute()
                && result
                    .output
                    .extension()
                    .is_some_and(|e| e == if result.diagnose { "json" } else { "png" }),
            "Diagnostic output must be absolute (.png for capture, .json for measurements)"
        );
        ensure!(
            ["library", "organize", "settings", "group"].contains(&result.screen.as_str()),
            "Unknown capture screen"
        );
        ensure!(
            [
                "empty",
                "populated",
                "error",
                "long-names",
                "ai-setup",
                "http-server"
            ]
            .contains(&result.scenario.as_str()),
            "Unknown capture scenario"
        );
        ensure!(
            result.scenario == "empty" || result.root.is_some(),
            "A populated capture needs an explicit fixture root"
        );
        Ok(Some(result))
    }
    pub fn page(&self) -> u8 {
        match self.screen.as_str() {
            "organize" => 2,
            "settings" => 3,
            _ => 1,
        }
    }
    pub fn layout(&self) -> Result<Layout> {
        let mut layout = Layout::default();
        // Capture bypasses startup work while showing the production default.
        layout.settings.startup_update_check = true;
        layout.settings.opacity = 255;
        if self.scenario == "http-server" {
            layout.settings.ai_base_url = "http://192.0.2.10:8080/v1".into();
        }
        if self.scenario == "empty" {
            return Ok(layout);
        }
        let root = self.root.as_ref().context("Fixture root required")?;
        crate::operations::checked_path(root, true)?;
        layout.settings.root = root.clone();
        let mut entries = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|e| e.file_name());
        ensure!(
            entries.len() <= 40,
            "Capture fixture is limited to 40 items"
        );
        for entry in entries {
            crate::operations::checked_path(&entry.path(), true)?;
            layout.import(entry.path(), entry.file_type()?.is_dir(), 1);
        }
        ensure!(!layout.items.is_empty(), "Capture fixture root is empty");
        if self.scenario == "long-names" {
            layout.items[0].name =
                "Quarterly planning notes with a deliberately long filename for layout review.md"
                    .into();
        }
        let groups = grouping::offline(&layout.items);
        grouping::apply(&mut layout, &groups)?;
        layout.validate()?;
        Ok(layout)
    }
}

pub(super) unsafe fn save(options: &Options, main: HWND) -> Result<()> {
    unsafe {
        let started = Instant::now();
        let target = if options.screen == "group" {
            APP.with(|cell| {
                cell.borrow().as_ref().and_then(|app| {
                    app.layout
                        .groups
                        .iter()
                        .find(|g| g.name == "Documents")
                        .or_else(|| app.layout.groups.first())
                        .map(|g| app.windows[&g.id].hwnd)
                })
            })
            .context("No group to capture")?
        } else {
            main
        };
        let (width, height) = if options.screen == "group" {
            (360, 300)
        } else {
            (1000, 760)
        };
        let (width, height) = (scale(width, options.dpi), scale(height, options.dpi));
        let mut outer = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        AdjustWindowRectExForDpi(
            &mut outer,
            GetWindowLongPtrW(target, GWL_STYLE) as _,
            0,
            GetWindowLongPtrW(target, GWL_EXSTYLE) as _,
            GetDpiForWindow(target).max(96),
        );
        SetWindowPos(
            target,
            null_mut(),
            0,
            0,
            outer.right - outer.left,
            outer.bottom - outer.top,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
        );
        // Automatic scrollbar visibility can change the client extent during WM_SIZE.
        // Correct that finite resize before rendering exactly the requested viewport.
        for _ in 0..3 {
            let mut client = RECT::default();
            GetClientRect(target, &mut client);
            if client.right == width && client.bottom == height {
                break;
            }
            let mut window = RECT::default();
            GetWindowRect(target, &mut window);
            SetWindowPos(
                target,
                null_mut(),
                0,
                0,
                window.right - window.left + width - client.right,
                window.bottom - window.top + height - client.bottom,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
            );
        }
        let mut client = RECT::default();
        GetClientRect(target, &mut client);
        ensure!(
            client.right == width && client.bottom == height,
            "Capture viewport did not settle"
        );
        let source = GetDC(target);
        ensure!(!source.is_null(), "Cannot create capture DC");
        let dc = CreateCompatibleDC(source);
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as _,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..zeroed()
        };
        let mut pixels = null_mut();
        let bitmap = CreateDIBSection(source, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
        ReleaseDC(target, source);
        ensure!(
            !dc.is_null() && !bitmap.is_null() && !pixels.is_null(),
            "Cannot allocate capture bitmap"
        );
        let old = SelectObject(dc, bitmap);
        SendMessageW(target, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
        if target == main {
            let controls: Vec<_> = APP.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .unwrap()
                    .controls
                    .iter()
                    .filter(|c| GetWindowLongPtrW(c.hwnd, GWL_STYLE) as u32 & WS_VISIBLE != 0)
                    .map(|c| c.hwnd)
                    .collect()
            });
            for child in controls {
                let mut r = RECT::default();
                GetWindowRect(child, &mut r);
                MapWindowPoints(null_mut(), main, &mut r as *mut RECT as *mut POINT, 2);
                SaveDC(dc);
                IntersectClipRect(dc, r.left, r.top, r.right, r.bottom);
                SetViewportOrgEx(dc, r.left, r.top, null_mut());
                SendMessageW(
                    child,
                    WM_PRINT,
                    dc as usize,
                    (PRF_CLIENT | PRF_NONCLIENT | PRF_ERASEBKGND) as isize,
                );
                RestoreDC(dc, -1);
            }
        }
        GdiFlush();
        let bgra = std::slice::from_raw_parts(pixels as *const u8, (width * height * 4) as usize);
        let mut rgb = Vec::with_capacity((width * height * 3) as usize);
        for pixel in bgra.chunks_exact(4) {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ensure!(
            started.elapsed().as_secs() < 10,
            "Capture exceeded its internal rendering deadline"
        );
        write_png(&options.output, width as _, height as _, &rgb)?;
        let metadata = serde_json::json!({"screen":options.screen,"scenario":options.scenario,"scope":"client area, production Win32 controls and shared GDI renderer","backend":"CPU GDI / native controls","pixels":[width,height],"dpi":options.dpi,"elapsed_ms":started.elapsed().as_millis(),"layouts":LAYOUTS.load(Ordering::Relaxed),"main_rasters":MAIN_PAINTS.load(Ordering::Relaxed),"group_rasters":GROUP_PAINTS.load(Ordering::Relaxed),"network_requests":0});
        fs::write(
            options.output.with_extension("capture.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )?;
        Ok(())
    }
}
fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("Capture path has no parent")?)?;
    let temporary = path.with_extension("png.pending");
    let file = fs::File::create(&temporary)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    writer.finish()?;
    fs::OpenOptions::new()
        .write(true)
        .open(&temporary)?
        .flush()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_rejects_unknown_routes_and_nonisolated_populated_mode() {
        for args in [
            vec!["--capture", "relative.png"],
            vec!["--capture", "C:/temp/a.png", "--screen", "typo"],
            vec!["--capture", "C:/temp/a.png", "--scenario", "populated"],
        ] {
            assert!(Options::parse(args.into_iter().map(OsString::from)).is_err());
        }
        assert!(
            Options::parse(
                ["--capture", "C:/temp/a.png", "--scenario", "empty"]
                    .into_iter()
                    .map(OsString::from)
            )
            .unwrap()
            .is_some()
        );
    }
}
