# Interface design

The intended result is a calmer desktop, with files easy to find and no surprise filesystem changes. The desktop groups are the everyday surface; the library is an occasional editing workspace reached from the tray. There is one settings entry point and no dashboard metrics.

The normal journey is **add a folder → preview groups → apply**. Grouping stays local by default. AI requires the selected-file sharing opt-in and an explicit request. File moves are a separate optional workflow with a complete preview. People can rename groups and reassign files without learning JSON, numeric IDs or model response schemas.

The initial technical form was replaced with a clear sidebar, a common type scale, aligned spacing, white work surfaces, a navy background role and a single blue action accent. Native selection and focus feedback remain available. Controls without a usable result are disabled; optional move controls stay out of the normal preview. Empty, error, populated and long-name states have been visually reviewed. Group headers and original document glyphs replace the extension-badge presentation.

The actual palette and drawing components live in `src/platform/ui/theme.rs`. The screenshots in `docs/screenshots/` come from the production executable's native components and renderer, not a second UI implementation. The per-image capture metadata documents viewport, scale, finite rendering and isolation. These images establish layout and appearance; they do not establish input handling or tray click behavior.
