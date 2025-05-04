# Architecture

## Components

| Module | Responsibility |
| --- | --- |
| `model` | Versioned settings/layout, stable numeric IDs, one-group-per-item invariant, metadata/type categories |
| `grouping` | Deterministic suggestions, exact selected-ID coverage, atomic visual application preserving unselected items |
| `ai` | Metadata allowlist, URL restrictions, bounded OpenAI-compatible request, strict untrusted JSON parsing |
| `operations` | Preview, root/name/reparse validation, fingerprints, source/directory locks, atomic no-replace rename, durable journal and undo |
| `persistence` | Bounded JSON reads and flushed atomic writes |
| `worker` | Two sleeping bounded workers/queues: serial file/state operations and serial network operations |
| `platform/ui` | UI-thread-owned Win32 settings and group windows, input/drop handling, GDI painting, tray, monitor/DPI behavior |
| `updates`, `platform/updater` | Stable release/asset selection, HTTPS download/digest/progress, wait/verify/replace/restart helper |

## Thread and lifecycle contract

The UI thread blocks in `GetMessageW` when idle. It does not continuously render or scan directories. Group painting occurs in `WM_PAINT`, uses a temporary double-buffered GDI bitmap, and clips/scans only displayed tiles. The current implementation uses linear lookup of item records (maximum 2000 references, 40 groups); no benchmark or measured CPU/latency improvement is claimed. Idle behavior and paint latency require interactive measurements.

One bounded channel serializes file operations, previews, saves and helper preparation. Another serializes network checks, AI and downloads. An opt-out startup update request cannot hold up file import or saving. Each worker blocks waiting for jobs and publishes results with a native wake message, including while settings are hidden. No UI state lock is held by a worker. Reentrant native dialog/menu callbacks use the default window procedure instead of aliasing the UI's mutable state; pending results are drained after the outer callback returns. `WM_DESTROY` quits even during a nested callback, without a paint. Explicit quit waits for its identified save result, while X hides settings. Window placement writes are debounced with a one-shot 450 ms timer, removed after firing.

Fullscreen detection uses out-of-context foreground/location events, filters location events to the foreground window, and coalesces pending notifications. There is no global keyboard/mouse hook, injected DLL, desktop parenting trick, permanent topmost flag, service or idle polling timer. Native groups belong to this process, not Explorer, and the tray re-registers on `TaskbarCreated`. Windows controls Show Desktop; an explicit Show groups restores the windows. Foreground rights are requested through normal Windows activation APIs.

## Storage and effects

No startup scan is performed and no default Desktop path is assumed. Import/drop inspect only user-selected items and add references. File opening uses the Windows association only on a user double-click/Enter. Filesystem moves require the separate option, edited suggestion, full preview and final confirmation. Paths and fingerprints remain local. A reparse-point-free ancestor chain is checked and held against rename/removal during moves; a source file handle excludes write/delete sharing before hashing and atomic rename. The journal records intent then completion and supports safe recovery, not a multi-file transaction. Runtime state is outside the portable executable directory, so updates preserve it.

API tokens are DPAPI blobs in settings; no plaintext token is serialized or intentionally included in errors. Metadata consent is session-only. AI request bodies are constructed from an explicit metadata struct that has no path/content field. Response validation authorizes group names and ID assignments only, with another independent folder-plan validator before filesystem effects.

## Primary API references

- [Win32 message loop](https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues)
- [SetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) and [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info)
- [Taskbar creation notification](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification)
- [SetWinEventHook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook)
- [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- [GitHub latest release API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)

Win32 signatures and ureq 3.5 APIs were also checked against the locally cached pinned crate sources. Shell hook registration/undocumented Explorer parenting was deliberately not used.
