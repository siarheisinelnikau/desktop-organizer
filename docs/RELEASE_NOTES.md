# DesktopOrganizer v0.1.0 — initial release draft

This is a local preparation draft. No repository or release was published in this session; there is no previous published baseline to compare.

## Added

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

Windows 10/11 x64 portable executable: `DesktopOrganizer-v0.1.0.exe`. MIT-licensed source; packaging includes checksum and third-party notices. Runtime uses Win32/GDI and the static MSVC runtime. No AI model or service is bundled. The EXE is unsigned.

## Verification and known limits

Compilation, Clippy, 19 synthetic-data tests, five real native component PNG captures and bounded visible/hidden/restore measurements passed. Thirty native page transitions required no layout or shared control-font recreation and left no unexpected controls visible. Native tray registration and system-close/restore dispatch were checked without input injection. Physical tray clicks, perceived flicker, keyboard traversal, dragging, mixed monitor/DPI, Show Desktop, Explorer lifecycle, real-provider compatibility and update/restart still require interactive testing. Do not describe this draft as an interactively verified production release.

Groups are independent non-topmost windows, not an Explorer wallpaper integration; original Desktop icons remain. Original file glyphs replace extracted Shell icons. Import is a manual snapshot, the first version has one active layout, and file moves support only direct ordinary files under one selected local root. Cloud/reparse points, UNC paths, cross-volume/recursive moves and elevated installation are unsupported. Undo stops on edits or occupied original paths and cannot resolve ambiguous crashes automatically.
