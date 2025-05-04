# Verification record and deferred checks

## Authorized automated checks

All commands run only in `E:/work/github/desktop-organizer`, with a repository-local Cargo target and at most two build jobs. Dependency resolution succeeded offline from the existing cache. Tests create temporary fixtures only under `.test-output/` in this repository, then clean up their own temporary directories. No real Desktop, user documents, GUI, updater process, live AI endpoint, device or Windows setting was operated.

The final validation commands are:

```powershell
./scripts/check.ps1
./scripts/package.ps1
python scripts/inspect_binary.py dist/DesktopOrganizer-v0.1.0.exe
git diff --check
```

The check script runs formatting, Clippy with warnings denied, library fixture tests and an optimized Windows x64 build. Test cases cover deterministic grouping and selected-item preservation, strict malformed/unknown/duplicate/incomplete AI output, URL/privacy serialization restrictions, DPAPI protection of an artificial token and invalid-blob refusal, semantic release ordering/asset provenance, invalid path/name rejection, preview collisions, late collisions with partial-operation undo, changed source/destination content, undo conflicts, interrupted-rename recovery, state replacement/corruption, and checksum-gated executable replacement using artificial bytes. It does not call the provider or GitHub API.

Final result on 2026-10-31: formatting, Clippy, all **16 library tests**, optimized Windows x64 compilation and packaging passed using Rust/Cargo 1.96.1. Static PE inspection confirmed x64, the Windows GUI subsystem, an embedded `asInvoker` / `PerMonitorV2` / Common Controls v6 manifest, and no dynamic MSVC/UCRT dependency. The EXE imports standard Windows system libraries. This inspection reads bytes only and does not establish that the UI is visually correct. The exact packaged checksum is in `dist/SHA256SUMS`; the build output is deliberately ignored by Git and can be reproduced with the packaging script.

## Environment limitations observed

- Atomic fixture renames in the external system Temp were denied in this sandbox. The same tests run successfully in repository-local fixtures; no elevation was requested.
- Cargo's automatic `clippy --fix` lock server could not bind/connect to its local socket (`os error 10013`). Fixes were made manually; ordinary Clippy is available.
- Incremental cache hard-link warnings can occur on this workspace filesystem; Cargo falls back to copying. They do not establish runtime performance.
- No live AI models/API, update downloads or GUI launches are part of these checks. External release availability is not probed during tests.

## Mandatory first interactive validation

Use a disposable folder of copied synthetic files, never the real Desktop for destructive tests. Keep backup copies of the layout and journal. The following checks remain unverified:

- Application and Settings screenshots showing the actual final executable, with English alt text/captions added to the README.
- Native layout, keyboard/tab navigation, text wrapping, scrolling and dark group colors at 100%, 125%, 150%, 200% DPI; access to every settings action on a small display using scrollbars.
- Real Explorer drops of file/folder/shortcut references, tile dragging between windows, large-group scrolling, Enter/double-click launching a harmless fixture and Delete removing references only.
- Rename, resize, collapse/expand, appearance changes and exact persistence after normal Save and quit/relaunch.
- A monitor left of the primary (negative coordinates), differing DPI values, unplugging/reconnecting a monitor, resolution changes and recovery of offscreen groups.
- Show Desktop/Win+D cycles and explicit tray Show groups, fullscreen enter/exit including F11 without a focus change, lock/unlock, virtual desktops and focus/z-order interactions with other apps. Groups are not wallpaper children and may be minimized by Windows.
- Tray click, tray Open, closing settings, repeated hide/restore, launch a second copy from hidden/minimized, and Quit while hidden. Confirm actual HWND visibility, focus, one process and process exit.
- Explorer restart: the existing groups survive and the tray icon returns, with no duplicate processes or orphan windows.
- Nonintrusive startup checks and the persistent opt-out: disabled means no startup request; manual checking still works. Check 404/unpublished, rate limit, offline, malformed response and available-update UI states with an isolated controlled test setup.
- Provider compatibility with a user-supplied token, including local ai-hub `http://localhost:8080/v1`, model `universal`; observe exactly the selected metadata and no path/content sharing. Verify malformed/provider-error handling and DPAPI across sessions/accounts. Do not record tokens.
- End-to-end portable updater on two controlled versions: progress, cancellation by closing the app only after completion, invalid/truncated digest, read-only installation, disk-full, antivirus/file locks, wait timeout, restart and retained settings. Verify backup/incoming/stage recovery and no incomplete installed binary.
- Reparse/junction/symlink attacks and source/destination races against disposable fixtures; ACL-denied files, cloud placeholders, busy files and non-NTFS filesystems. Narrow unit tests verify the regular-file rename path but do not establish all Windows filesystem semantics.
- Measure release-binary CPU/thread waits while visible idle, tray-hidden idle, scrolling and dragging; record frame/input latency with representative fixture counts. Neither GDI nor a sleeping event loop alone proves perceptual smoothness.
- Windows logoff/shutdown or forced termination during the 450 ms save debounce can lose the latest unsaved visual change; explicit Save and quit waits for persistence. Exercise interrupted file operations and resolve ambiguous journals conservatively.

## Publication gate

No remote Git operation or publication was authorized. Before a future release, complete the interactive checks, capture both required screenshots, confirm canonical repository ownership and version/asset digest, review dependency notices and produce accurate release notes. Use the connected GitHub plugin for any separately authorized remote action.
