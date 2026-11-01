# DesktopOrganizer v0.2.0

This is the first published release of DesktopOrganizer.

## Initial capabilities

- Arrange desktop file references in named movable groups with saved layouts, tile order, collapse controls and tray access.
- Preview deterministic grouping or optional OpenAI-compatible metadata suggestions before explicitly organizing selected files.
- Journal real filesystem moves and offer conservative undo. Floating groups are ordinary native windows; the application does not replace Explorer or embed groups into the wallpaper layer.

## Localization and appearance

Choose your interface language and switch between light and dark appearance directly in Settings.

## Added

- English, Russian, French, German, Spanish and Italian interfaces, including navigation, organization and move review, settings, update actions and progress, tray commands, empty states and validation messages.
- Language and Theme selectors at the top of Settings. Changes take effect immediately and save automatically, without restarting the application.
- A coherent dark palette for the workspace, native input controls, lists, action states and floating groups. Light appearance now also applies to floating groups.
- Developer capture options for each language and theme, plus native diagnostic coverage of all six languages and both themes.

## Preserved

- Existing settings and layouts load with English and Light defaults. File names, paths, existing group names and AI-provided names are preserved. Newly generated offline folder names use the selected interface language.
- Reviewed file moves and undo, optional metadata-only AI, startup-update opt-out and user-initiated verified update installation keep their existing behavior.
- Event-driven Win32/GDI rendering and the retained main scene remain in use; changing language or theme schedules a finite refresh.

## Platform and verification

Windows 10/11 x64 portable asset: `DesktopOrganizer-v0.2.0.exe`, with `SHA256SUMS`, MIT license and third-party notices. The executable is unsigned.

See [current verification](VERIFICATION-v0.2.0.md) for exact automated checks and bounded native evidence. Windows-owned dialog buttons, menus and non-client chrome can follow the operating system's language and appearance; technical diagnostic details retain their source text. Physical tray interaction, keyboard traversal, mixed-monitor behavior, perceived compositor smoothness and actual update installation remain separate manual checks.

The source comparison baseline is local v0.1.2. Publication and the preceding published release are verified separately by the release workflow; these notes do not claim that a release has already been published.
