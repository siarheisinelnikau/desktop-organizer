# DesktopOrganizer v0.3.0 verification

Verified locally on November 2, 2026, against the local v0.2.1 baseline. No release or repository upload was performed.

## Build, artifacts and regression scope

`./scripts/check.ps1` passed with Rust 1.96.1: formatting, strict Clippy for all targets, 49 library tests and optimized compilation. Cargo used locked offline dependencies, two jobs and this repository's target directory. `./scripts/package.ps1 -SkipBuild` prepared the x64 portable executable, SHA256SUMS, MIT license and complete dependency notices, including the pinned Windows COM bindings.

The final executable is 4,836,864 bytes. SHA-256: `04b359bd9edac78cadbc437130eec6957e6c807289c29e8dab29ed0f4d6ad2a0`. Static inspection confirms x64, Windows GUI subsystem, asInvoker, PerMonitorV2, Common Controls v6, GDI/native-control imports and the Windows version-resource API. The existing icon resources were loaded from this exact EXE as data, rendered at native 16/20/24/32/48/64 pixel sizes and visually inspected on light/dark backgrounds; resources through 256 pixels are present. [Icon evidence](verification/icons-v0.3.0.json) is separate from OS tray/taskbar interaction.

Focused fixtures cover:

- Creating and reading a genuine synthetic Shell link without launching its inert target; target identity survives renaming the shortcut.
- Reading this product's embedded executable version resource as data.
- URL host/protocol extraction while excluding credentials, URL paths/queries and identity hashes from the AI allowlist.
- Preference serialization/reload, source/target identity matching, and local enforcement when the model proposes a different category.
- Real loopback HTTP streaming through the production request path, public-summary deltas, ordinary JSON fallback, incomplete/oversized response rejection and ignoring reasoning fields.
- Seventy-shortcut grouping/refinement, exact ID coverage, subset refinement, preserved unrelated groups and error handling.
- Monitor-cell minimum geometry, reverse selection, overlap checks, edge clamping, retained regions after regrouping and layout serialization.
- A mixed filesystem move/undo fixture in which only the document category moves; the screen-category shortcut remains at its original path with unchanged contents.

These are synthetic and loopback fixtures. They do not measure the quality of a live model on the user's actual Desktop.

## Actual component captures

`./scripts/capture-workflows.ps1 -SkipBuild` reproduces seven isolated render-save-exit captures using 16 inert fixture files under `.local/PurposeDesktop`. Every invocation has a 15-second external deadline and exits after saving its real Win32/GDI components. All seven resulting PNGs were opened and visually inspected:

- English Light: categories/mixed destinations, library with Arrange on screen, combined path review and Settings/version 0.3.0.
- Russian Dark: mixed destinations, AI conversation and monitor region editor with Games at the top center.

Adjacent `.capture.json` files record the backend, viewport, locale/theme and zero capture-time network requests. The public model response and received-character count in the conversation image are explicit synthetic UI state; no live completion was used. Initial review found opaque white text backgrounds, a clipped conversation button and missing hidden-combo selections. The final captures use the shared palette/owner-draw path, transparent GDI text backgrounds, native combo state and corrected control spacing. The preference reset lives in the conversation footer, leaving the provider form unobstructed.

These captures establish client-area layout and text, including mixed category labels and full move paths. They do not establish physical mouse/keyboard operation, native popup-menu behavior or live streaming latency. Earlier supplemental README screenshots keep their original fixture/version metadata.

## Native rendering and lifecycle evidence

Source review confirms CPU GDI/native Win32 controls, blocking GetMessage, sleeping worker queues, event-driven network progress and no recurring dialog/render timer. Hidden conversation windows retain state without refreshing native text controls. Dialog parent/native-child paint requests and layouts have separate counters; captures deliberately produce finite print requests.

The final packaged EXE ran the bounded `--diagnose` fixture under a 48-second external deadline as PID 53564. It verified both modeless dialogs, applied a valid region layout to a cloned model, checked scan/proposal/review without moving files, exercised 30 native page transitions and removed its own tray icon before exiting. [Full report](verification/native-v0.3.0.json) records the exact executable and counters.

| State/workload | Sample | Process CPU, one core | UI thread, one core | Main/group rasters | Dialog paints/layouts |
| --- | --- | --- | --- | --- | --- |
| Visible, light pointer traffic | 5 s | 0.00% | 0.00% | 0 / 0 | 0 / 0 |
| After navigation, light pointer traffic | 5 s | 0.00% | 0.00% | 0 / 0 | 0 / 0 |
| Hidden, no input | 5 s | 0.00% | 0.00% | 0 / 0 | 0 / 0 |
| Restored, accessibility traffic | 5 s | 6.87% | 3.12% | 0 / 0 | 0 / 0 |
| Restored follow-up, accessibility traffic | 5 s | 10.00% | 5.94% | 0 / 0 | 0 / 0 |

Main/group/layout/cached-blit work was zero in all five samples. Ten deliberate unchanged exposures rebuilt zero scenes. The 30 page transitions created zero fonts, caused zero layouts/unexpected visibility and dispatched in at most 11.17 ms; this is dispatch timing, not presentation latency.

The restored samples received 1,230 and 1,809 WM_GETOBJECT messages plus related query traffic. Windows uses [WM_GETOBJECT for accessibility object requests](https://learn.microsoft.com/en-us/windows/win32/winauto/wm-getobject). They are active accessibility workloads, not quiet idle measurements. The querying client's identity is unknown; accessibility remains enabled. An earlier candidate's [trace](verification/native-before-wait-fix-v0.3.0.json) prompted correction of the diagnostic pump's wait-for-already-observed-input flag. The normal production loop already blocks in GetMessage. Workload differences prevent treating these runs as a controlled CPU performance comparison.

Rendering stops in the measured visible/hidden/restored states, but a traffic-free restored CPU baseline, compositor/GPU timing and perceived interaction latency remain unverified. No global Windows setting, unrelated process or accessibility feature was changed. Physical region dragging, tray clicks and cross-monitor interaction were not automated. No actual Desktop file, live AI provider, updater download/install or real program target was used in these checks.

## Product boundaries

Screen categories are independent non-topmost panels. Their references preserve original paths, and ordinary Explorer icons are not hidden or replaced. The mixed file plan moves only categories explicitly left in Folder mode, with the existing final confirmation and undo restrictions. Local preference memory is not model training, and unavailable shortcut/program evidence remains uncertain.
