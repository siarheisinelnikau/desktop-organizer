# DesktopOrganizer v0.1.2 — local validation draft

This patch follows the local v0.1.1 build. No repository or release was published in this session; there is no previous published baseline to compare.

## Changed in v0.1.2

- Show groups becomes Hide groups while the windows are shown; the same button can hide and reopen them. Explicit tray show/hide actions remain available.
- Save & test AI sends a small chat to the configured model without file metadata. Settings distinguish untested, checking, verified green Connected and failed states; editing credentials/address/model invalidates the checked state, and stale responses cannot turn it green again.
- Organize is now the opening workflow: Scan my Desktop reads existing files and immediately proposes folders. Review file moves opens a full original-path/destination screen; moving originals still needs explicit count confirmation. Existing directories and other-source references are excluded. Floating visual groups are a secondary option and start hidden.
- Tiles can be inserted before another tile or appended on a background drop, within and between groups. The order persists; selected tiles also have context-menu and Alt+Up/Down commands.
- A synthetic shortcut move/undo test verifies that only the shortcut file moves and its target remains unchanged. Main-window scenes are retained across unchanged exposures; CPU rasterization and cached blits are measured separately.

## Changed in v0.1.1

- HTTP AI servers are allowed, including LAN addresses. A brief inline warning appears for HTTP and disappears for HTTPS; no transport confirmation is required.
- AI access-denied, missing-endpoint and rate-limit failures explain the next action without exposing provider response bodies or tokens.
- GitHub 404 is a distinct public-release-unavailable state, with a useful explanation instead of a raw transport error. It is never reported as up to date.
- The sidebar contains navigation without another product logo/name/version block. The native title is simply DesktopOrganizer. Empty status cards and repeated idle messages are removed.
- Synthetic HTTP tests exercise the actual network worker, DPAPI token, request metadata, response validation and group application, plus failure and redirect handling. The native diagnostic also verifies saving HTTP settings and the startup-check opt-out.

## Existing capabilities

- Native, resizable visual desktop groups with original file glyphs, a draggable header, tile transfer, collapse/expand and saved monitor-relative placement.
- A library and tray/settings workflow with a light sidebar, original line icons and file/category badges, shorter actions, disabled unavailable actions, a percentage opacity slider, accent control, fullscreen hiding policy and a single running instance. Optional AI setup opens on demand.
- Deterministic offline organization and optional selected-metadata AI suggestions using a configurable OpenAI-compatible service. The group preview supports renaming and reassignment without JSON; API tokens are protected with Windows DPAPI.
- Optional root-confined file organization with a complete move preview, collision avoidance, fingerprint checks, an operation journal and conflict-aware undo.
- Startup update checking with a persistent opt-out, manual checking, and a user-initiated download/verify/replace/restart workflow prepared for the canonical GitHub repository.

## Fixed during local validation

- Page changes keep existing control fonts and geometry, batch visibility updates and buffer native drawing. Refresh no longer makes lists from hidden pages visible. The active page is highlighted on first opening as well as after navigation.
- Adding a folder imports it directly; refreshing it keeps existing manual group assignments.
- Long labels are ellipsized at a readable size, group actions have enough width, and finite PNG captures settle to the requested viewport without a blank bottom strip.

## Platform and packaging

Windows 10/11 x64 portable executable: `DesktopOrganizer-v0.1.2.exe`. MIT-licensed source; packaging includes checksum and third-party notices. Runtime uses Win32/GDI and the static MSVC runtime. No AI model or service is bundled. The EXE is unsigned.

## Verification and known limits

Compilation, Clippy, 25 tests, eight real component captures and bounded Desktop/settings/visibility measurements passed. A scan of seven synthetic loose files opened a full move review without touching an existing directory or another-folder reference. Four show/hide commands, 30 page transitions and ten unchanged exposures passed; settled samples built no scenes, and hidden operation performed no blits. Connection UI fixtures and actual probe transport against loopback were tested separately. Physical input/drag gestures, compositor latency, mixed monitors, real-provider compatibility and end-to-end update/install still require separate testing.

Groups are independent non-topmost windows, not an Explorer wallpaper integration; original Desktop icons remain. Original file glyphs replace extracted Shell icons. Import is a manual snapshot, the first version has one active layout, and file moves support only direct ordinary files under one selected local root. Cloud/reparse points, UNC paths, cross-volume/recursive moves and elevated installation are unsupported. Undo stops on edits or occupied original paths and cannot resolve ambiguous crashes automatically.
