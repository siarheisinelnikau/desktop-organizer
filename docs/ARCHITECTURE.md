# Architecture

## Components

| Module | Responsibility |
| --- | --- |
| `model` | Versioned settings/layout, stable numeric IDs, one-group-per-item invariant, metadata/type categories |
| `metadata` | Read-only saved Shell-link targets, safe EXE version-resource strings and URL host/protocol evidence; no target launch |
| `memory` | Bounded explicit preferences keyed by local SHA-256 target/reference identities, local enforcement after model output |
| `regions` | Validated monitor-relative cells, minimum dimensions, overlap and pixel geometry |
| `purpose`, `grouping` | Conservative complete-word application-name hints, deterministic purpose/type suggestions, exact selected-ID coverage, atomic visual application preserving unselected items |
| `ai` | Metadata allowlist, URL restrictions, bounded OpenAI-compatible request, strict untrusted JSON parsing |
| `streaming` | Bounded SSE/public-content decoding, partial public-summary decoding and ordinary JSON fallback |
| `operations` | Preview, root/name/reparse validation, fingerprints, source/directory locks, atomic no-replace rename, durable journal and undo |
| `persistence` | Bounded JSON reads and flushed atomic writes |
| `worker` | Two sleeping bounded workers/queues: serial file/state operations and serial network operations |
| `platform/ui` | UI-thread-owned Win32 settings and group windows, input/drop handling, GDI painting, tray, monitor/DPI behavior |
| `updates`, `platform/updater` | Stable release/asset selection, HTTPS download/digest/progress, wait/verify/replace/restart helper |

## Thread and lifecycle contract

The UI thread blocks in `GetMessageW` when idle. It does not continuously render or scan directories. Group painting occurs in `WM_PAINT`, uses a temporary double-buffered GDI bitmap, and clips/scans only displayed tiles. The current implementation uses linear lookup of item records (maximum 2000 references, 40 groups); no benchmark or measured CPU/latency improvement is claimed. A bounded native diagnostic records scene rasterization, owner-draw, layout and CPU counters in visible, hidden and restored states. See VERIFICATION.md for measured conditions; input/presentation latency is not inferred from these counters.

One bounded channel serializes file operations, previews, saves and helper preparation. Another serializes network checks, AI and downloads. An opt-out startup update request cannot hold up file import or saving. Each worker blocks waiting for jobs and publishes results with a native wake message, including while settings are hidden. No UI state lock is held by a worker. Unhandled messages reach DefWindowProc only after the mutable App borrow has been released. This lets synchronous SC_CLOSE-to-WM_CLOSE and resize reentry reach the real handlers. Pending worker results are drained after native dialogs/menus return. `WM_DESTROY` quits even during a nested callback, without a paint. Explicit quit waits for its identified save result, while X hides settings. Window placement writes are debounced with a one-shot 450 ms timer, removed after firing.

Fullscreen detection uses out-of-context foreground/location events, filters location events to the foreground window, and coalesces pending notifications. There is no global keyboard/mouse hook, injected DLL, desktop parenting trick, permanent topmost flag, service or idle polling timer. Native groups belong to this process, not Explorer, and the tray re-registers on `TaskbarCreated`. Windows controls Show Desktop; an explicit Show groups restores the windows. Foreground rights are requested through normal Windows activation APIs.

## Storage and effects

No startup scan is performed and no default Desktop path is assumed. Import/drop inspect only user-selected items and add references. File opening uses the Windows association only on a user double-click/Enter. Filesystem moves require the separate option, edited suggestion, full preview and final confirmation. Paths and fingerprints remain local. A reparse-point-free ancestor chain is checked and held against rename/removal during moves; a source file handle excludes write/delete sharing before hashing and atomic rename. The journal records intent then completion and supports safe recovery, not a multi-file transaction. Runtime state is outside the portable executable directory, so updates preserve it.

API tokens are DPAPI blobs in settings; no plaintext token is serialized or intentionally included in errors. Metadata consent is session-only. AI request bodies are constructed from an explicit metadata struct that has no path/content field. Response validation authorizes group names and ID assignments only, with another independent folder-plan validator before filesystem effects.

Application hints use names plus available read-only program evidence. Unknown applications use Needs review; recognized shortcut target extensions can identify document references. The AI allowlist includes a tentative purpose_hint and selected program details, while omitting the old extension-only Applications category. The system prompt requests semantic purpose/topic groups and explicitly treats all supplied names/hints/proposals as untrusted data.

AI grouping makes one initial request, then at most one bounded refinement request when a generic folder exceeds eight items or a folder exceeds twelve items with at least two name hints for each of at least two purposes. Only candidate folders' items and previous assignments enter refinement. Exact subset coverage, group-count capacity, strict JSON and whole-selection coverage are validated; equal folder names are merged with retained folders. Still-broad or invalid refinements fail without replacing the current preview. Each grouping request uses a 120-second transport timeout on the sleeping network worker. No recurring scan, model call or UI timer is introduced.

AI base URLs accept HTTP and HTTPS. The provider panel updates a short unencrypted-transport warning when the address changes; saving is not blocked for HTTP. URL credentials/query/fragment remain disallowed, and requests never follow redirects. A bounded loopback HTTP fixture tests the production network worker through validated group application with a synthetic token.

Update checking uses the shared core. Its current API returns textual failures, so the host maps the exact pinned GitHub 404 failure to a separate public-release-unavailable state. Other failures remain errors; neither becomes an up-to-date result. This adapter is covered by a regression test and does not alter shared download or installation behavior.

## Primary API references

The v0.3.0 pipeline inspects selected items on background workers. Shell-link loading uses IPersistFile and IShellLinkW GetPath/GetDescription without Resolve. Arguments are used only for a local identity hash, distinguishing launchers that target the same EXE; raw arguments are not persisted or shared. Local fixed-drive EXE version strings use GetFileVersionInfoSizeW/GetFileVersionInfoW/VerQueryValueW with a bounded buffer and checked resource ranges. Unsupported or inaccessible targets retain partial/uncertain evidence. Full target paths never enter the AI metadata struct.

The request allowlist adds program filenames, product/description/company strings, shortcut descriptions, website host/protocol and selected-item preferred categories. The prompt asks for a public summary followed by strict groups. SSE delta.content is bounded and coalesced to at most ten progress updates per second; reasoning fields are ignored. An event carries only decoded public summary text and a received-character count. Ordinary JSON responses remain supported. Each pass has a 120-second transport deadline. Failed or truncated responses never replace the current preview.

Corrections and grouping guidance live in the existing atomic layout file. Explicit per-program choices are applied locally after response validation and excluded from automatic mixed-group refinement. A target hash survives an ordinary shortcut rename; a source-reference hash provides a fallback. Regrouping retains placements for surviving category names. No fine-tuning, background provider polling or inference service is installed.

Folder/on-screen choices are persisted by category name. The organization planner creates an exact subset for folder categories, preserving the existing root/fingerprint/journal/undo restrictions; on-screen IDs never enter that file plan. Visual application happens after successful folder moves, using current item records. An independent preflight validates full grouping before filesystem work. A mixed move review lists both retained screen categories and exact source/destination paths.

The modeless conversation and region editor use native controls and CPU GDI. Hidden conversation windows retain data without updating native text controls. Both windows have event-driven painting and no recurring timer. Their paint/layout counters join the bounded native diagnostic. The 8 × 6 cell editor fits minimum dimensions of 280 × 240 logical pixels, prevents overlaps and refuses a layout with unplaced categories. Group tiles keep the existing fixed geometry and scroll; normal window dragging returns a group to free placement. Saved regions are recomputed against the chosen monitor's work area after display changes.

- [Shell links and IPersistFile loading](https://learn.microsoft.com/en-us/windows/win32/shell/links)
- [Executable version-resource queries](https://learn.microsoft.com/en-us/windows/win32/api/winver/nf-winver-verqueryvaluew)

- [Win32 message loop](https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues)
- [SetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) and [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info)
- [Taskbar creation notification](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification)
- [SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook)
- [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- [GitHub latest release API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)

Win32 signatures and ureq 3.5 APIs were also checked against the locally cached pinned crate sources. Shell hook registration/undocumented Explorer parenting was deliberately not used.

## Visual system and bounded diagnostics

`platform/ui/theme` owns the palette, typography, spacing, original line icons and native owner-drawn controls. Organize previews real Desktop folder moves; My groups is the secondary floating-reference workspace. Full file review and final count confirmation precede filesystem changes; provider fields open through AI settings. Group windows share logical tile coordinates for painting/hit testing. Native strings, selection and base keyboard mechanisms are preserved; screen-reader traversal still needs interactive review.

Page navigation changes visibility in one redraw transaction, then invalidates the native descendants once. Shared control fonts are cached by DPI; control geometry changes only for resize, scrolling or the optional setup panel. Main scenes use a GDI back buffer and the main window enables native descendant buffering through WS_EX_COMPOSITED. Hidden lists are not passed WM_SETREDRAW(TRUE), which would implicitly make them visible. None of these mechanisms schedules recurring frames.

`platform/ui/capture` creates an isolated instance before normal single-instance activation, settings loading or startup jobs. It invokes the shared WM_PRINTCLIENT/GDI scene and the actual native controls into a DIB, then encodes that bitmap as PNG and exits. No desktop screenshot or alternate mock renderer is involved. `--diagnose` uses the same executable and native event dispatch, not input injection. Its timers/waits are finite and exist only in that developer mode. Normal operation has no recurring render timer.

The explicit AI probe reuses the chat transport with a small constant prompt and no item metadata. Its worker result carries a UI revision; edits advance that revision and reset the state, preventing a late result from validating changed fields. Connection state is session-only and never triggers background polling. Save & test saves the current fields before queuing the probe. Model/key/base edits clear the indication; appearance-only saves preserve it.

Organize opens with Scan my Desktop instead of an import/selection workflow. It resolves FOLDERID_Desktop only on user invocation, scans on the file worker and automatically proposes offline folders. The planner maps its native list through current-source item IDs, excluding directories, other-source references and stale references outside the latest scan. The general library still retains references. No startup scan or automatic AI request is added. File review has its own page with the complete paths and a separate confirmation; floating groups remain secondary and start hidden.

The main background scene is retained in a compatible GDI bitmap keyed by viewport, DPI, scroll, page and structural UI state. Changed content rebuilds it; unchanged WM_PAINT/WM_PRINTCLIENT requests blit the retained scene. Native children keep their own control semantics. Scene rasters, cached blits, paint/print requests, model refreshes and status updates have distinct diagnostic counters. A ten-exposure regression verifies no unchanged-scene reconstruction.

Tile order lives in each group's item-ID vector. The same placement operation supports within-group insertion and cross-group transfer; a background drop appends. The native context menu and Alt+Up/Down use the model's relative-order operation. Reordering a reference does not inspect or launch its target; scanning and AI requests read saved shortcut metadata separately. Normal file-plan validation remains necessary for explicit filesystem effects.
