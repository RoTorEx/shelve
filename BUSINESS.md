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
domain-specific or root-folder assumption. The bundled starter config contains
only standard macOS locations; personal destinations belong only in the user's
config.

The `open` and `move` flows remain local. Network access is limited to the
explicit `update` command and installation from GitHub Releases.
