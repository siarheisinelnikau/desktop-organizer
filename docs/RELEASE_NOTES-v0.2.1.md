# DesktopOrganizer v0.2.1

Local update prepared against the repository's v0.2.0 baseline. Publication has not been requested.

## Improved

- Recognizable applications now receive purpose-based offline folder suggestions, including Browsers, Development, Design & 3D, Games, Communication, audio/video tools and other uses. Unknown names remain in Needs review.
- AI requests include a concrete purpose-based task and examples. They no longer label every executable and shortcut as Applications before asking the model.
- Large generic or mixed-purpose AI folders receive one additional focused grouping pass, preserving other proposed folders. Invalid or repeatedly broad results report an error while keeping the existing preview.
- Purpose labels appear in the selected-file list and are translated into all six supported UI languages.

## Verification and limitations

- Regression fixtures cover 70 shortcuts, exact ID coverage, focused second-pass requests, retained-folder preservation and duplicate-folder merging. No live provider quality measurement is claimed.
- Names are the available evidence. Shortcut targets, file contents and executable descriptions are not inspected; unknown names can require manual review.
- Windows 10/11 x64 portable asset: `DesktopOrganizer-v0.2.1.exe`. Existing settings and move/undo behavior are preserved. See [verification](VERIFICATION-v0.2.1.md) for build, rendering and runtime evidence.
