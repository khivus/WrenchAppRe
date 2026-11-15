# v0.9.0-preview.3 verification

This prerelease combines window layout and display settings in one Save workflow,
adds shared Undo/Redo/Revert, offers edited-setting sync to matching characters
and accounts, and restores Fleet chat to the offline preview.

## Automated verification

Formatting, strict all-target Clippy and workspace tests run locally and in the
release workflow. The workflow builds Windows, Linux and both macOS architectures
and verifies all nine established release packages before publication.

## Manual acceptance

Desktop appearance and live EVE acceptance remain unverified. With a backed-up
profile and EVE logged out:

- Change an overview setting and move a window; Save once, reopen, and confirm both.
- Undo/Redo mixed display and geometry edits; Revert should restore both drafts.
- Save account-only, character-only and mixed edits; check the sync prompt offers
  matching targets, Cancel preserves targets, and applying preserves unrelated settings.
- Check Fleet chat standalone and stacked, including closed/minimized states.
- Follow [the overview checklist](overview-editor-verification.md) and
  [the editor checklist](editor-verification.md) for related regressions.
- Start each platform package and confirm version 0.9.0-preview.3.

Public notes: [v0.9.0-preview.3](releases/v0.9.0-preview.3.md).
