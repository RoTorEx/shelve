# Shelve

Shelve is a small, local macOS CLI for three jobs:

- `shelve open` is a universal shortcut to important configured folders and
  opens the selected folder in Finder.
- `shelve move` lets the user choose destinations for PDFs, previews the batch,
  and moves it after one confirmation.
- `shelve update` downloads the latest compatible GitHub Release, verifies its
  SHA-256 checksum, and atomically replaces the running installation.

It never uploads documents or overwrites an existing file. It is intentionally
not a document classifier, archive index, watcher, or document-management
framework.

Open locations are entirely config-driven. Shelve has no built-in
domain-specific or root-folder assumption. Config version 2 defines sections
with explicit roots and required item lists. Relative items belong to the
section root; home-relative and absolute items are valid only when they still
resolve below that root. A section is a filesystem umbrella and cannot contain
unrelated folders. An empty item list exposes only the root selector. Legacy
flat `[[locations]]` configs remain readable. The bundled starter config
contains only standard macOS locations; personal destinations belong only in
the user's config.

The `open` and `move` flows remain local. Network access is limited to the
explicit `update` command and installation from GitHub Releases.

Folder navigation follows Hop: a scrollback menu with lettered groups,
numbered folders, visible spacing, and typed selectors such as `A1` followed by
Enter. `shelve open A1` opens a known folder without the menu. Blank input, EOF,
or `q` cancels; invalid selectors report an error without selecting a fallback.

Sections follow config order, and items are sorted by full resolved path within
each section. Duplicate item paths are shown once. Shelve performs no automatic
folder discovery: the config is the exact visible navigation surface, including
items at any depth. Letters continue after Z as AA, AB, etc.; folder positions
are one-based. Selectors are positions, not permanent IDs. Open and move use the
same materialized folders and codes without per-folder eligibility flags.

Section headers and folder names come from actual path components, never custom
labels. Version 2 headers use the configured root rather than inferring a
common ancestor from the number or placement of entries. Legacy groups retain
their inferred-root behavior. Headers show the root name followed by its parent
path, matching Hop. `<letter>0` opens that root without adding a numbered root
entry; folders start at 1. Root shortcuts are open-only and cannot be used to
select a move destination. Deeper items show their parent relative to the
section root.

The menu follows Hop’s spacing and color roles, with a dim version and dividers,
a parent path in each sector heading, and a single short input prompt. Usage
help stays in documentation rather than appearing below every menu.

Use Hop’s exact ANSI roles: bright blue (94) for heading and prompt marker,
bold (1) for the prompt action, and dim (2) for the selector placeholder.

Bare `shelve` shows help and never opens a picker. `shelve move` treats discovered
PDFs only as candidates: the user must choose individual source codes before
any destination questions. This applies to inbox scans and explicit file or
directory arguments. Empty input or EOF cancels; no source is selected by
default. Multiple codes are allowed, duplicates are removed, and any invalid
code rejects the entire selection. Folder-root codes cannot select files.

Source and destination menus use the same renderer and prompt styling as open.
Source headings say Files; destinations say Folders, with the source filename
on a separate line so long filenames do not expand the divider. Home paths
use `~` consistently. Multi-file codes remain space- or comma-separated.
