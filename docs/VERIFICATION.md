# Verification record

## Current scope

Work stays in `E:/work/github/desktop-organizer`, with two Cargo jobs and a local target. Fixtures contain synthetic files only. No real Desktop items were moved, no AI provider was called, and no updater/install helper or remote Git operation was executed. After the user requested no computer use, visual review used only bounded application self-capture and native in-process diagnostics.

## Automated checks

Rust/Cargo 1.96.1: formatting, Clippy with warnings denied, **18 library tests**, optimized Windows x64 compilation and packaging passed. Tests cover grouping/selected-item preservation, metadata serialization privacy, malformed/unknown/duplicate model output, URL restrictions, DPAPI on a synthetic token, release selection, root/name validation, no-overwrite moves, changed content, collision/undo/recovery, atomic persistence, capture argument isolation and group tile hit testing.

The executable is inspected without execution by `scripts/inspect_binary.py`: x64, Windows GUI subsystem, embedded `asInvoker` / `PerMonitorV2` / Common Controls v6 manifest, and no dynamic MSVC/UCRT dependency. The packaged checksum is in `dist/SHA256SUMS`.

```powershell
./scripts/check.ps1
./scripts/package.ps1
python scripts/inspect_binary.py dist/DesktopOrganizer-v0.1.0.exe
./scripts/capture.ps1
git diff --check
```

## Actual visual evidence

The four PNGs in `docs/screenshots/` are production Win32 controls plus the shared CPU GDI renderer. They show populated library, editable group preview, settings and a document group, at 100% scale. They are client-area captures, excluding OS title bars/menus and the physical taskbar. Each process saved its PNG and capture metadata, exited with code 0, made no startup network request, and did not load or save normal user settings.

Additional local captures reviewed empty/error states and a long filename at 125% scale. Fields/actions stayed accessible; a long row uses horizontal scrolling rather than shrinking text. These are deliberate developer scenarios, not claims that the UI was driven by real input. The empty state now explains the next action; unavailable operations are disabled; optional filesystem controls are hidden until selected. The source uses the same tile geometry for painting and pointer hit testing.

## Rendering/load gate evidence

Source/configuration review: native Win32 controls and GDI CPU drawing, no wgpu/OpenGL/DirectX renderer. Normal dispatch sleeps in GetMessage. Save debounce is one-shot. Workers block in bounded queues; network and file work use separate workers. Paint does not self-invalidate. Hidden/minimized WM_PAINT validates its region without building/rasterizing a scene. Capture mode explicitly renders a finite offscreen frame and exits.

Final bounded diagnostic: optimized `target/release/DesktopOrganizer.exe`, PID **21648**, six synthetic files, one second of settling followed by five seconds of sampling per state. This native diagnostic used no mouse/keyboard injection or computer-use API. See `diagnostics/runtime.json` for the original report.

| State | Sample | CPU seconds | One-core CPU | Main/group rasters | Layouts / owner draws |
| --- | --- | --- | --- | --- | --- |
| Visible settled | 5.012 s | 0 at process timer resolution | 0% | 0 / 0 | 0 / 0 |
| Hidden settled | 5.002 s | 0 at process timer resolution | 0% | 0 / 0 | 0 / 0 |
| Restored settled | 5.006 s | 0.09375 | 1.87% (0.094% machine-wide) | 0 / 0 | 0 / 0 |

No sustained scene generation occurred in these settled windows. The native system-close path preserved the actual HWND and hid settings to tray; the production restore dispatcher made the window visible again. Tray registration succeeded outside the sandbox. The process exited normally after the bounded run. The earlier sandbox diagnostic could not register a tray icon, but kept settings accessible as designed.

This passes the narrow native idle/hidden/restore check. It does **not** establish pointer/scroll latency, physical tray clicks, focus behavior during a real user action, all native-control internals, GPU/compositor timing or universal absence of stutter. GPU telemetry was not collected. Do not infer interaction performance from the PNGs or CPU sample.

## Repairs verified at the appropriate level

- DefWindowProc now runs after releasing App state, allowing synchronous system-close/resize reentry to reach custom handlers. The SC_CLOSE-to-WM_CLOSE close/restore path was checked by the bounded native diagnostic.
- Tray registration is deferred until the dispatcher is ready. Failure keeps settings available and prevents an inaccessible hide; TaskbarCreated can retry.
- Show groups raises non-topmost group windows. Fullscreen detection includes transitions back into this process.
- Collapsed groups no longer rasterize clipped file tiles. Group header/tile layout shares the tested hit-test grid.
- The consumer preview uses names and direct reassignment instead of JSON/IDs. Disabled, empty and error presentation was reviewed from actual component captures.

## Remaining interactive/hardware checks

Do not use the real Desktop for destructive checks. The following remain separate from the static/native evidence above:

- Physical tray click/menu, repeated user-driven hide/restore and duplicate launch, keyboard/tab/screen-reader traversal, resize gestures, collapse gestures, appearance controls and actual focus/z-order behavior.
- Explorer drops, cross-window tile dragging, large-group scrolling, item launching and reference removal through real input.
- Mixed DPI/negative-coordinate monitors, unplug/reconnect, virtual desktops, Show Desktop, fullscreen/F11 transitions, lock/unlock and Explorer restart.
- Live AI-provider compatibility with user-supplied credentials, runtime startup-update opt-out/network observation, and end-to-end update download/install/restart on controlled releases.
- Junction/symlink races, cloud placeholders, ACL/file-lock edge cases and non-NTFS behavior beyond the synthetic regular-file tests.
- Input/presentation latency and compositor/GPU evidence with a representative workload. Windows shutdown/forced termination during the 450 ms debounce can still lose the latest unsaved visual change; normal Quit waits for persistence.

## Environment notes

External system Temp atomic renames were denied during the original code-only pass, so fixtures use repository-local directories. Clippy automatic-fix socket creation was denied; ordinary Clippy and manual fixes work. Cargo occasionally warns that its incremental hard links fall back to copying. No escalation, driver/settings changes or unrelated-process termination was used to work around these build limitations.

No repository/release publication was authorized. Complete the remaining interactive checks and review version, checksum, dependencies and release notes before any separately authorized publication through the GitHub plugin.
