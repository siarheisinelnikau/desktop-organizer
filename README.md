# DesktopOrganizer

Organize the files and shortcuts already on your Windows Desktop into a mix of reviewed folders and persistent on-screen categories. Optional floating groups and AI suggestions are also available. This independent, GPLv3-licensed application uses native Win32 controls and GDI; it has no connection to Stardock, uses no competitor artwork and requires no WebView, service, model download or AI subscription.

**Version 0.3.0 · Windows 10/11 x64.** The interface is available in English, Russian, French, German, Spanish and Italian, with persistent light and dark themes. Automated verification and bounded native diagnostics are described below. Physical tray clicks, drag-and-drop and broader hardware behavior still need interactive validation.

## What it does

- Named, movable, resizable group windows with persisted layouts and tile order, collapse/expand, mouse-wheel scrolling, opacity and an accent color.
- Import references from an explicitly selected folder, or drop files/folders from Explorer onto a group. Drag tiles between groups; double-click or press Enter to open an item. Delete removes a reference only.
- Tray entry for settings, show/hide groups and explicit save-and-quit; closing settings keeps the app in the tray. A second launch requests activation of the existing instance.
- Per-monitor DPI awareness, monitor-relative placement, clamping to the available work area after monitor changes, and optional hiding while the foreground window covers its monitor.
- Program-aware offline purpose hints for recognizable application names (Browsers, Development, Design & 3D, Games, Communication and more), with **Needs review** for unknown applications and type categories for other files. Optional OpenAI-compatible suggestions group by purpose/topic and reconsider overly broad proposals once. The plain-language preview supports rename and file reassignment.
- Optional explicit filesystem organization: every move is previewed, constrained to the chosen source root, fingerprinted, journaled and undoable when original paths remain free and content remains unchanged.
- Manual and nonblocking startup update checks, a persistent startup opt-out, and user-initiated verified portable update installation with progress, backup, atomic replacement and restart.

Groups are ordinary **non-topmost tool windows**, independent of Explorer. This version does not embed them into the wallpaper layer, hide original Windows Desktop icons, or replace Explorer. Groups use original file glyphs rather than extracted Shell icons. Show Desktop is left to Windows; if it minimizes/hides the group windows, use **Show groups** in the tray/settings. Explorer's `TaskbarCreated` broadcast recreates the tray icon. These shell behaviors require the manual checks listed below.

## Build and package

Requires an existing Rust 1.95+ MSVC x64 toolchain and Windows SDK/linker; the validated toolchains are Rust 1.96.1 and 1.99.1. The target is Windows 10/11 x64. Older Windows and other executable architectures are unsupported. Runtime uses standard Windows APIs and a statically linked MSVC runtime; no application DLL bundle is needed.

```powershell
cd E:\work\github\desktop-organizer
# Once, when dependency downloading is permitted and they are not already cached:
cargo fetch --locked
./scripts/check.ps1
./scripts/package.ps1
# Optional static inspection with Python 3 (never loads/executes the EXE):
python scripts/inspect_binary.py dist/DesktopOrganizer-v0.3.0.exe
```

The shared updater workspace must be present at `../rust-auto-updator`; its core crate is a local path dependency, not a published package. CI runners must provision that sibling workspace before Cargo resolution. Standalone repository checkout is insufficient until the shared workspace is published or separately supplied.

Cargo uses at most two jobs and this repository's `target/`. Verification and packaging use `--offline` and never launch the GUI, updater helper, or any item. HTTP integration tests use a bounded server on `127.0.0.1` with synthetic tokens; no real AI provider is contacted. The output is `dist/DesktopOrganizer-v0.3.0.exe`, with `LICENSE`, `SHA256SUMS` and third-party license texts. This is one executable architecture, so its asset name has no architecture suffix. The binary is currently unsigned; no release or signing certificate is implied.

## Use

1. Start the EXE in a writable portable folder and choose **Scan my Desktop**. The file worker inspects the selected folder, saved shortcut targets and available executable descriptions. It never launches a target. Scanning does not move files.
2. Review **Categories**. Each row shows **Folder** or **On screen**. Select a category and toggle **Keep this category on screen** to choose its destination. Rename categories or reassign selected items as needed; these explicit corrections are saved and respected on future scans and AI requests.
3. **Ask my AI** requires the session-only **Share names and program details with my AI** consent. It opens **AI conversation** automatically. The main screen and dialog show metadata inspection counts, waiting, received-answer character counts, refinement and validation. The dialog streams the model's public explanation when the provider supports streaming. Enter a follow-up grouping instruction and choose **Send & regroup** to request a new reviewed proposal. Guidance is saved locally for future requests; conversation history is session-only. Nothing moves from a chat message.
4. Choose **Review organization**. For a mixed plan, the review lists categories kept on screen followed by every original file path and destination for folder categories. **Move N files** still requires final confirmation. Screen categories are excluded from filesystem moves. When every category is on screen, review opens the region editor directly. **All categories on screen** is a shortcut for that choice.
5. In **Arrange categories on screen**, select a category and monitor, then drag a rectangle across the 8 × 6 grid. For example, place Games in the top center and Development on the left. The minimum region protects readable tiles; item size stays fixed and overflowing items scroll. Arrow keys move a selected region. Regions cannot overlap on the same monitor. **Auto arrange** fills available regions; if space runs out, use another monitor or put some categories into folders. **Save & show groups** persists monitor-relative regions. Reopen the editor through **My groups → Arrange on screen**. Manually moving/resizing a group returns it to free placement.
6. **Undo moves** reverses verified completed folder moves when paths and content still permit it. It does not move the files belonging to screen categories. Floating panels show references to originals and do not change their targets.
7. **Settings** contains language, theme, optional AI connection and update preferences. **AI conversation → Forget grouping choices** clears saved corrections and conversational grouping guidance. The normal tray actions show/hide groups and quit; closing settings keeps the app in the tray.

Settings and references live in `%LOCALAPPDATA%\DesktopOrganizer\layout.json`; the current filesystem journal is `operation.json`. Layout writes use a flushed temporary file and atomic replacement. A malformed/future layout is preserved and startup reports an error instead of replacing it. An existing operation journal is reconciled in the file worker on startup so references follow verified completed moves; ambiguous states are reported for manual attention. Back up this directory while the app is closed if you need layout history. The first version has a single active layout, not multiple named layout profiles.

## Optional AI and privacy

Example for an independently configured ai-hub service:

```text
Base URL: http://localhost:8080/v1
Model: universal
New API token: <your own token>
```

Open **Settings → AI settings**, enter the provider and your own token if required. **Save settings** stores configuration; **Save & test AI** saves it and sends a small test chat to the selected model, without filenames or library metadata. The status distinguishes **not tested**, **checking**, green **Connected**, and **connection failed**. Editing connection fields invalidates the checked state; an old response cannot mark the changed configuration connected. Status is the last check in this session, not continuous monitoring, and starts unverified after a restart. Successful grouping requests also confirm access.

A blank token field keeps the existing saved token; **Remove saved token** clears it. DPAPI encrypts the saved token for the current Windows user. Do not copy encrypted tokens between accounts. No provider secrets are included in this repository, read from other applications, or auto-discovered.

HTTP and HTTPS servers are supported, including an HTTP server on another LAN computer. HTTP displays **HTTP is unencrypted. Avoid public Wi-Fi.** under the address; it does not block saving or require another confirmation. Prefer HTTPS when the network cannot be trusted. DPAPI protects the saved token on disk, not an HTTP connection. API URLs must not embed credentials, queries or fragments.

Before each sharing session, explicitly enable **Share names and program details with my AI**. Only selected item IDs, names, extensions, kinds, tentative purpose hints, saved preferred category names, your grouping guidance and an allowlist of program details are sent. Program details are the target filename/extension, product name, file description, company, shortcut description, website host and protocol when available. Full source/target paths, raw command-line arguments, URL paths/credentials/queries, local identity hashes, file contents, icons and unselected records are excluded. Redirects remain disabled. Consent is not persisted.

The model receives purpose-based examples, program evidence and your saved preferences. A large generic or mixed folder gets at most one focused refinement request for affected items. Explicit saved choices are also enforced locally, so a model response cannot silently override them. Responses must be strict JSON with optional `summary` and required `groups`, `name`, `item_ids`; every supplied ID must appear exactly once. No model text executes as a command. Invalid, truncated or repeatedly broad responses preserve the previous preview.

Grouping requests ask for streaming and have a 120-second transport deadline per pass. The dialog shows the public summary as it arrives, then validated folders. Providers that return ordinary JSON instead of a stream remain supported: they show the waiting stage and final explanation. The UI shows actual stages/counts rather than an invented percentage. Hidden dialogs do not refresh their controls; network work remains on a background worker.

Local `.lnk` inspection loads the saved Shell link without calling Resolve or launching the target. Available EXE descriptions come from Windows version resources, read as data on fixed local drives after path checks. Remote, reparse/cloud and unavailable targets are not probed for descriptions; the saved target name or shortcut name remains usable. `.url` inspection reads the stored destination and shares only host/protocol. Raw arguments are hashed locally to distinguish launchers using the same EXE for different games; the arguments themselves are neither saved nor shared. Missing facts remain uncertain rather than fabricated.

Manual proposal renames, file reassignments and cross-group tile moves save per-program preferences in the layout. Target identity allows an ordinary shortcut rename to retain the preference; the source-reference identity is a fallback. This is explicit preference memory, not model fine-tuning. A different target/version path may require another correction. **Forget grouping choices** is available in the conversation dialog.

## File moves and undo

Scanning and suggestions never move files. The primary Desktop workflow previews actual file moves:

1. Choose **Scan my Desktop**, then review/edit the automatically proposed folders.
2. Choose **Review organization**. A separate review screen lists every original path and destination, including collision suffixes. **Back to folders** returns to editing; review again after changing the proposal.
3. Review all moves and choose **Move N files**, then confirm the count. Changes to sources after preview cause a refusal, rather than moving unknown content.
4. **Undo moves** loads/reconciles the journal and reverses completed moves, including after a restart. Changed destinations or newly occupied original paths stop undo safely. Resolve such conflicts manually with the journal preserved, then retry.

Only ordinary files directly inside the chosen source folder can be moved into one-level group subfolders of that same root. Folders can be grouped visually but are not recursively moved. UNC/device paths, alternate data streams, traversal, symbolic links and reparse-point ancestors are rejected. Windows directory handles exclude write/delete sharing to prevent ancestor substitution; source content is verified through a handle locked against writers/deleters, followed by an atomic no-replace rename by handle. Collisions use numbered names in preview; new collisions after preview stop without overwriting. Cross-volume moves, cloud placeholders/reparse points and elevation are intentionally unsupported.

`.lnk` and `.url` shortcuts are references: grouping/reordering changes only their visual placement; a confirmed filesystem move moves the shortcut file itself. Its target program or document is not moved. A synthetic `.url` move/undo test verifies that the target remains unchanged.

Journal entries are flushed before and after each rename. An interrupted operation with exactly one matching source/destination can be reconciled. Ambiguous cases (both/neither path, or edited content) require manual resolution; this is not a transactional rollback across all files. Earlier completed moves can be undone after a later move fails. Only empty directories recorded by the operation are removed during undo. A previous journal with completed moves must be undone/resolved before another file operation can replace it.

## Updates

**Check updates** uses the public stable release API for the canonical repository without GitHub login. **Check for updates on startup** is enabled by default; save this toggle to disable startup requests. Startup checks do not block launch, show intrusive failure/no-update dialogs, download or install automatically. Available updates remain visible in Settings. Missing/unpublished releases, rate limits and connectivity errors are reported as unavailable/error, not as up-to-date.

GitHub 404 is shown as **Public releases are unavailable**, with an explanation that the repository may be private or have no published release. This does not mean the installed version was confirmed current.

**Download & install** uses the shared `rust-auto-updator` core with this product's canonical repository/version/asset configuration. A release must include `DesktopOrganizer-vVERSION.exe` and `SHA256SUMS` for that exact asset. The core bounds the transfer, verifies SHA-256 and the x64 PE structure, stages beside the installed executable, and reports progress. Any GitHub-provided digest must agree with the checksum file. The application saves its layout before handing the verified update to the shared helper, and exits only after handoff succeeds. The helper waits for the original process, verifies its identity and unchanged installed bytes, atomically replaces the EXE with a backup, and restarts it. Settings and user files are never replaced.

Installation needs a writable EXE directory and no administrator rights. The shared staging folder uses `.desktop-organizer-update-` beside the executable; successful startup removes only known staged updater files. Preserve a failed staging folder/backup for investigation. A helper error is also recorded in `%LOCALAPPDATA%\DesktopOrganizer\update-error.txt`. No update download, helper launch or actual application replacement was executed during this audit.

## Verification and screenshots

See [v0.3.0 verification](docs/VERIFICATION-v0.3.0.md), [architecture](docs/ARCHITECTURE.md), and the [v0.3.0 release notes](docs/RELEASE_NOTES-v0.3.0.md). CI performs noninteractive Windows compilation and fixture tests, then uploads build artifacts. Release publication uses a separate explicitly triggered workflow.

The new mixed workflow and dialogs below are real client-area renders from the optimized executable, using explicit synthetic fixture state. The conversation image contains a synthetic public response; no live provider was called.

![Mixed folder and on-screen categories in Russian Dark](docs/screenshots/organize-mixed-ru.png)

![Monitor cell editor with Games placed at the top center](docs/screenshots/arrangement-ru.png)

![AI conversation showing the public response and measured receiving stage](docs/screenshots/conversation-ru.png)

The following production-rendered client areas use a separate synthetic application fixture. The proposals are generated offline from names, without calling a model or opening a shortcut.

![Purpose-based folders for recognizable applications, with unknown shortcuts left for review](docs/screenshots/organize-purpose.png)

![Russian purpose-based folder proposal in the dark theme](docs/screenshots/organize-purpose-dark-ru.png)

The images below were generated by this executable from its production GDI renderer and native Win32 controls, using explicit synthetic fixtures. Current main-workspace images use the 16-file purpose fixture; older supplemental images retain their recorded seven-file fixture and version metadata. They show client areas at 100% scale, with an 820-pixel-high Settings client area and 760 pixels for the main workspace. They are not desktop photographs and do not certify OS-native tray menus or input behavior.

![Desktop group with three document references](docs/screenshots/group.png)

![Populated library with group management and file references](docs/screenshots/library.png)

![Desktop folder proposal with a direct Review file moves action](docs/screenshots/organize.png)

![Opening Organize screen with Scan my Desktop for existing files](docs/screenshots/organize-empty.png)

![Full original-path and destination review before confirming file moves](docs/screenshots/moves.png)

![Settings with opacity slider, optional AI connection and updates](docs/screenshots/settings.png)

The following real component captures show the same settings in Russian with Dark appearance and German with Light appearance. The native selectors display the saved choices.

![Russian settings with dark appearance and persistent language and theme selectors](docs/screenshots/settings-dark-ru.png)

![German settings with light appearance](docs/screenshots/settings-de.png)

![Russian organization preview with dark appearance](docs/screenshots/organize-dark-ru.png)

![AI settings with a synthetic HTTP address, brief transport warning and unavailable public release state](docs/screenshots/settings-ai.png)

The following is the real application's explicit synthetic success scenario for reviewing the connected indicator; no real provider was contacted to generate this image.

![Green connected indicator in the synthetic connection-success scenario](docs/screenshots/settings-connected.png)

Reproduce these bounded render-save-exit captures with `./scripts/capture.ps1`. Each invocation renders its own components into a CPU bitmap, writes a PNG plus `.capture.json` metadata, and exits without tray registration, startup update requests or access to normal user settings. The script never automates the desktop.

Developer-only examples (use absolute output and fixture paths):

```powershell
./target/release/DesktopOrganizer.exe --capture E:/work/github/desktop-organizer/.local/review.png --screen settings --fixture-root E:/work/github/desktop-organizer/.local/SampleDesktop
./target/release/DesktopOrganizer.exe --capture E:/work/github/desktop-organizer/.local/empty.png --screen library --scenario empty
./target/release/DesktopOrganizer.exe --diagnose E:/work/github/desktop-organizer/.local/runtime.json --fixture-root E:/work/github/desktop-organizer/.local/SampleDesktop
```

Capture also accepts `--language en|ru|fr|de|es|it` and `--theme light|dark`; each capture records these selections in its metadata. Settings captures use an 820-pixel-high client area to include the new appearance controls.

Capture supports `library`, `organize`, `moves`, `settings`, `group`, scenarios `populated`, `empty`, `error`, `long-names`, `ai-setup`, `http-server`, `ai-connected`, `no-preview`, and scale 100/125/150. The HTTP/connected scenarios select synthetic UI states. Diagnostic mode verifies settings, current-folder scan/proposal/review without moving files, connection indication, four show/hide transitions, 30 page transitions and ten unchanged exposures. It samples visible/hidden/restore/follow-up states, separates rasters from cached blits and UI CPU from process CPU, records message types, removes its tray icon and exits. Apply an external timeout (15 seconds for capture, 48 for diagnosis). Physical input, dragging, perceived flicker and compositor latency need separate checks; see the verification record for accessibility-traffic measurements.


The Windows executable and native small tray handle now share embedded multi-size artwork. This is a resource rendering proof from the actual optimized EXE, not a screenshot of the OS tray:

![Embedded application and small tray icon resource proof](docs/screenshots/embedded-icons.png)

The [v0.2.0 verification record](docs/VERIFICATION-v0.2.0.md) documents the current checks. The [earlier verification record](docs/VERIFICATION.md) and icon metadata retain their original version labels; shell interaction limits remain separate from resource proofs.
