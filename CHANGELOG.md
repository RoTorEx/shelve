# Project Changelog

Tracks real product and release progress.

## [Unreleased]

### Changed

- Ask which source PDFs to move before choosing destinations; no files are preselected.
- Bare `shelve` shows help again; use `shelve open` for folder navigation.

- Matched Hop’s bright-blue headings and prompt marker, bold action, and dim selector placeholder.

- Matched Hop menu formatting, parent-path headings, and concise input prompt.

- Display actual filesystem names and support opening group roots with selectors such as A0.

- Made bare `shelve` open the menu and added filesystem context to group and folder labels.

- Replaced the two-pane picker with a Hop-style menu: lettered groups, numbered folders, spaced sections, and typed selectors.
- Added direct `shelve open A1` navigation; PDF destinations retain their open-menu codes.

## [0.1.0] - 2026-08-31

### Added

- Added `open` with a grouped, terminal-adaptive folder picker and bounded navigation.
- Added safe PDF batch planning and moves without overwriting existing files.
- Added universal starter locations driven entirely by user configuration.
- Added `update`, `-V`, and checksum-verified GitHub Release installation for Apple Silicon and Intel macOS.
