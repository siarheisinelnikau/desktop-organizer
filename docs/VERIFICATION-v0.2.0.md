# DesktopOrganizer v0.2.0 verification

## Scope

This release adds six interface languages and persistent light/dark appearance. English remains the complete fallback and the migration default. Existing layout names and paths remain data; only newly generated offline folder names use the selected language.

The localization audit covers the primary workspace, organization and move review, settings and update controls, status and progress messages, validation text, tray commands and floating-group instructions. Technical error details, file paths, existing group names, AI-supplied names and Windows-owned dialog buttons retain their original/system language. The embedded catalog contains 182 messages with five complete translation rows each.

## Automated checks

- `cargo fmt --all --check`
- `cargo clippy --locked --offline --all-targets -- -D warnings`
- `cargo test --locked --offline --lib`: 29 tests passed, including catalog/placeholder coverage, parameter preservation, user-data preservation and existing-settings migration.
- `cargo build --locked --offline --release`
- `./scripts/package.ps1 -SkipBuild`
- `python scripts/inspect_binary.py dist/DesktopOrganizer-v0.2.0.exe`: x64, Windows GUI subsystem, asInvoker, PerMonitorV2, Common Controls v6 and a GDI/native-control import path.

All checks passed using Rust 1.96.1 and again with current stable Rust 1.99.1 (`cargo +1.99.1 ...`). The final packaged executable and native evidence below use 1.99.1. The final 1.99.1 fixture run disabled test debug symbols and incremental caching to bound build-cache disk use; all 29 tests and assertions passed. Strict Clippy used its normal profile. The newer Clippy chunk-iteration lint has been addressed with `as_chunks`.

The package contains `DesktopOrganizer-v0.2.0.exe`, `SHA256SUMS`, MIT license and complete dependency notices. The executable remains unsigned. Actual download, executable replacement and updater restart were not run.

## Real component captures

`./scripts/capture.ps1 -SkipBuild` generated eleven finite render-save-exit captures from the optimized executable. Every capture exited successfully and its PNG was visually inspected. The files and adjacent capture metadata are in `docs/screenshots/`:

- English Light: library, organization, empty organization, full move review, Settings, AI setup, synthetic connected state and a floating group.
- Russian Dark: Settings and organization.
- German Light: Settings, including longer translated captions.

The Settings selector metadata confirms six language choices, two themes and the requested selected indices. Hidden native combo boxes omit their selection in `WM_PRINT`; capture explicitly invokes the production owner-draw routine on the actual native selection. This is the same control state and GDI renderer used in normal operation. No desktop image, browser automation or artificial UI mockup is involved. The images establish client-area layout, typography, palette and translated text, not OS popup-menu behavior.

The review corrected an opacity/fullscreen-label overlap and shortened translated captions where the existing compact actions would otherwise clip. The executable's original icon was resource-loaded and rendered at native 16/20/24/32/48/64-pixel sizes on light and dark backgrounds; all embedded sizes through 256 pixels were present. See [icon resource evidence](verification/icons-v0.2.0.json) and the README's real resource proof. This is separate from physical taskbar/tray interaction.

## Native lifecycle and load gate

Source inspection confirms blocking `GetMessage`, event-driven worker completions and foreground notifications, one-shot debounced saves, a retained main scene and genuine CPU GDI/native controls. Language/theme changes invalidate a finite set of surfaces and save preferences without discarding a reviewed move plan. No continuous repaint timer or GPU renderer was introduced.

The bounded diagnostic ran the packaged optimized executable with synthetic files in an explicit isolated directory. A shared named mutex serialized visible diagnostics with other projects. The test exited successfully and removed its tray icon. [Full native report](verification/runtime-v0.2.0.json) records PID 42860 and the exact executable path.

| Settled state | Sample | One-core CPU | Main/group rasters | Cached main blits |
| --- | --- | --- | --- | --- |
| Visible | 5 seconds | 0.0000% | 0 / 0 | 0 |
| After navigation | 5 seconds | 2.4999% | 0 / 0 | 2 |
| Hidden | 5 seconds | 0.0000% | 0 / 0 | 0 |
| Restored | 5 seconds | 0.0000% | 0 / 0 | 0 |
| Restored follow-up | 5 seconds | 0.0000% | 0 / 0 | 1 |

The follow-up blit corresponds to one OS paint/exposure and reused the cached scene. Ten deliberately invalidated unchanged exposures rebuilt zero scenes. Thirty page transitions created zero fonts, caused zero layouts or unexpected control visibility, and dispatched in at most 10.18 ms in this run; this is native dispatch timing rather than presentation latency.

Native checks passed for all six language selections and both themes without restarting, preservation of reviewed moves when switching themes, saving HTTP settings and the startup-check opt-out, synthetic DPAPI token roundtrip, connection-state invalidation, import/proposal/review without moving files, four group show/hide transitions, close-to-tray, restore and cleanup. No live AI request or actual file move was performed by the diagnostic.

The rendering/load gate passed for this isolated native workload. GPU/compositor timing was not collected. Physical tray clicks, keyboard traversal, dragging, mixed monitors, subjective smoothness, real-provider compatibility and end-to-end updates remain separate manual checks.
