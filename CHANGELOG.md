# Project Changelog

Tracks real product and release progress.

## [Unreleased]

### Changed

- Replaced the two-pane picker with Hop-style lettered sectors, numbered entries, actual folder names, and parent paths.
- Unified open and move menu colors, spacing, short prompts, and home-relative paths; long source filenames appear on a separate line.
- Added `shelve open A1` for direct navigation and `<sector>0` to open a group root without listing it as an entry.
- Require explicit source-file selection before asking for PDF destinations. Multiple file codes are supported, nothing is preselected, and cancelling preserves all files.
- Keep destination codes consistent between open and move while enforcing `move_here` restrictions.
- Bare `shelve` displays help; folder navigation uses `shelve open`.

## [0.1.0] - 2026-08-31

### Added

- Added `open` with a grouped, terminal-adaptive folder picker and bounded navigation.
- Added safe PDF batch planning and moves without overwriting existing files.
- Added universal starter locations driven entirely by user configuration.
- Added `update`, `-V`, and checksum-verified GitHub Release installation for Apple Silicon and Intel macOS.
