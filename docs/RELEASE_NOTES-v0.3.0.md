# DesktopOrganizer v0.3.0

Prepared against the local v0.2.1 baseline. No GitHub publication is implied.

## Added

- Read-only analysis of saved shortcut targets, available executable product names/descriptions/companies, and internet-shortcut hosts/protocols. Renamed shortcuts can be identified from the target rather than just their visible name.
- Persistent grouping preferences from explicit category renames, reassignments and cross-group moves. Conversational grouping guidance is saved too; the model cannot override saved per-program choices. The conversation dialog includes a reset action.
- An AI conversation window with inspection counts, request/refinement/validation stages, streamed public explanations and follow-up grouping instructions. JSON IDs and internal reasoning are kept out of the conversation.
- Mixed organization: choose Folder or On screen per category, review the combined plan, then move only the folder categories with the existing confirmation and undo protections.
- A monitor region editor with an 8 × 6 grid, rectangular selection, collision checks, automatic placement, keyboard movement and persistent monitor-relative regions. Minimum dimensions protect readable fixed-size tiles; overflow scrolls.

## Improved

- Regrouping preserves saved category positions and screen modes.
- Missing monitors fall back to an available monitor; saved region proportions adapt to the available work area.
- Streaming and ordinary JSON responses are supported with bounded transfers and per-pass deadlines. Provider failures preserve the previous proposal.
- New workflow labels are localized into all six existing UI languages. Existing updater and portable data behavior are retained.

## Verification and limits

See [verification](VERIFICATION-v0.3.0.md) for regression fixtures, actual component captures and bounded native measurements. No live provider was contacted. These are independent floating panels; the application does not replace Explorer's desktop or hide its original icons. The portable x64 asset is `DesktopOrganizer-v0.3.0.exe`.
