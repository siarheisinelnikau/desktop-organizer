# DesktopOrganizer v0.2.1 verification

Verified locally on November 2, 2026. Changes address the extension-only Applications bucket and under-specified AI grouping task. No release was published.

## Build and regression checks

`./scripts/check.ps1` passed with Rust 1.96.1: formatting, strict Clippy for all targets, 37 library tests and optimized Windows compilation. Cargo used two jobs, locked offline dependencies and this repository's target directory. `./scripts/package.ps1 -SkipBuild` prepared the x64 portable EXE, checksum, the project license and dependency notices.

The tests reproduce 70 recognizable shortcuts being assigned to one Applications group, validate focused refinement into seven purposes, preserve a separate Reports folder, reject foreign/duplicate/missing IDs, merge a refined folder with a retained folder of the same name, reject a repeatedly broad answer after exactly two requests, and avoid unnecessary refinement for a good first answer or a homogeneous Games collection. Additional fixtures cover punctuation, version/copy suffixes, Russian names, unknown programs and false substring matches. HTTP fixtures use bounded loopback servers with synthetic credentials; these results test the pipeline and validation, not a live model's classification accuracy.

`python scripts/inspect_binary.py dist/DesktopOrganizer-v0.2.1.exe` confirmed an x64 Windows GUI binary with asInvoker, PerMonitorV2, Common Controls v6 and native GDI imports. The executable is 4,571,648 bytes; SHA-256 is `5fc725c01366f7418e26d86224761f732dfc9333684b0f6a8364165b931ab5ee`.

The packaged icon was loaded as data and rendered through DrawIconEx at 16/20/24/32/48/64 pixels on light and dark backgrounds. Embedded sizes through 256 pixels are present, and the resulting native-size contact sheet was visually inspected. [Resource evidence](verification/icons-v0.2.1.json) is distinct from OS taskbar/tray interaction.

## Real component captures

The optimized executable's isolated `--capture` mode rendered and saved these production Win32/GDI client areas, then exited successfully within the external 15-second timeout:

- `docs/screenshots/organize-purpose.png`: English Light, 1000 × 760 pixels.
- `docs/screenshots/organize-purpose-dark-ru.png`: Russian Dark, 1000 × 760 pixels.
- `docs/screenshots/settings.png`: English Light, 1000 × 820 pixels, installed version 0.2.1.

All three PNGs were opened and visually inspected. Adjacent `.capture.json` files record language/theme, backend, zero network requests and the bounded render. A separate fixture under `.local/PurposeDesktop` contains 14 inert shortcut files and two documents; names include Firefox, Chrome, VS Code, PyCharm, Blender, Fusion 360, Steam, Telegram, Discord, VLC, Resolve, Audacity, 7-Zip and Unknown launcher. None of these references is executed or resolved. Offline proposals contain 11 folders, with the unknown launcher in Needs review. Labels fit the existing layout in both inspected languages/themes. Other README captures retain their earlier fixtures/version metadata.

## Rendering and lifecycle gate

Source inspection established CPU GDI/native Win32 controls, blocking GetMessage, sleeping file/network workers, retained-scene blits and a one-shot save debounce. This change adds no recurring rendering, scanning or provider requests. No pre-existing DesktopOrganizer instance was running before diagnostics.

The optimized binary ran `--diagnose` with the explicit synthetic fixture and isolated output directory under a 48-second external timeout. PID 48660 exited successfully and removed its diagnostic tray icon. [Full native evidence](verification/native-v0.2.1.json) records the path and all counters. The packaged EXE is copied from that same optimized binary.

| Settled state | Sample | Recorded one-core CPU | Main/group rasters | Layouts | Cached main blits |
| --- | --- | --- | --- | --- | --- |
| Visible | 5.00 s | 0.0% | 0 / 0 | 0 | 0 |
| After navigation | 5.01 s | 0.0% | 0 / 0 | 0 | 0 |
| Hidden | 5.01 s | 0.0% | 0 / 0 | 0 | 0 |
| Restored | 5.01 s | 0.0% | 0 / 0 | 0 | 0 |
| Restored follow-up | 5.01 s | 0.0% | 0 / 0 | 0 | 1 |

CPU values reflect the sampler's timer resolution. The final cached blit corresponds to one OS WM_PAINT and did not rebuild the scene. Ten deliberately invalidated unchanged exposures produced zero new scene rasters. Thirty native page transitions created zero fonts, caused no unexpected visibility or layout, and took at most 18.37 ms per dispatch in this run. This is dispatch timing, not presentation latency. Native checks covered scan/propose/review without moving, 11 applied visual groups, settings persistence, six languages/both themes, close-to-tray, restore and cleanup.

No computer-use automation, live AI service, actual Desktop files, updater download/install or real shortcut targets were used. GPU/compositor timing, physical tray clicks, keyboard traversal and live-provider semantic quality remain unmeasured. Name-based hints and bounded refinement improve the available proposal logic; they do not establish correct classification of every real application.
