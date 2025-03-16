# DesktopOrganizer

Repository: `desktop-organizer`
Priority: 1 of 15 (user-requested launch order)
Product: DesktopOrganizer
Purpose: Desktop groups with optional AI-assisted organization
Inspiration: https://www.stardock.com/products/fences/

Independent open-source implementation; no affiliation, competitor code or copied assets.

## Requested implementation

Build a polished Windows desktop organizer inspired by Fences: named resizable groups of desktop shortcuts/files, drag-and-drop, collapse/expand, appearance controls, persisted layouts, multi-monitor/DPI-aware placement, a tray/settings entry, and deliberate behavior around Show Desktop/fullscreen/Explorer restarts. Prefer a real usable vertical slice over a large mock dashboard.

The user's extra requirement is essential: AI suggests groups based on filenames, extensions, item kind and icon/type categories. Provide configurable OpenAI-compatible base URL, model and API key. Include an ai-hub example using base URL http://localhost:8080/v1 and model universal, with a user-supplied token placeholder. Never read or copy ai-hub secrets or call a live endpoint during this unattended run. Provide deterministic offline grouping as a fallback.

Default grouping is visual and does not move files. An explicit 'Create folders and move files' option may generate a previewed filesystem plan. Show proposed groups and every move before Apply; the user can edit them. Confine destinations to the user-selected root, reject traversal and unsafe reparse-point traversal, handle collisions without overwrites, keep an operation journal and undo where feasible. Treat AI output as untrusted structured suggestions, never commands. Send only selected metadata after explicit opt-in. Test grouping, malformed model output, plan validation, collision handling and undo on fixtures, never the actual Desktop.

## Packaging and publication target

Windows standalone asset: `DesktopOrganizer-vVERSION.exe` (for the initial release, `DesktopOrganizer-v0.1.0.exe`). Append architecture only when distributing multiple executable architectures in the same release.
Planned canonical repository: https://github.com/siarheisinelnikau/desktop-organizer (not yet created or published; keep availability/error reporting truthful).

## Working agreement

Follow AGENTS.md. Complete as much real implementation and safe noninteractive verification as possible in this unattended code-only session. Manual UI, device and hardware tests will happen later. Record required external components and their verification status. Keep changes committed locally in meaningful increments.

