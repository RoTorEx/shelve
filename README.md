# shelve

A tiny macOS CLI for opening important folders and shelving PDFs:

```text
shelve open
shelve move [FILE_OR_DIRECTORY ...]
shelve update
```

`open` chooses a configured folder and opens it in Finder. `move` finds PDFs,
asks for a destination for each file, prints one batch preview, and moves only
after confirmation. Existing files are never overwritten.

Check the installed version with `shelve -V`. `shelve update` downloads the
latest compatible macOS release, verifies its checksum, and replaces the
current binary.

The folder picker groups destinations in a two-pane terminal interface. Use
`↑`/`↓` (or `j`/`k`) to move, `Enter` to select, and `Esc` to cancel. Navigation
stops at the first and last destination instead of wrapping around. The neutral,
high-contrast palette follows the terminal's own foreground and background.

## Install from GitHub

```sh
curl -fsSL https://github.com/RoTorEx/shelve/releases/latest/download/shelve-install.sh | sh
```

The installer supports Apple Silicon and Intel macOS, verifies the release
archive checksum, installs to `~/.x-cli-shelve/bin`, and preserves an existing
configuration.

## Build and install locally

```sh
make install-local
```

This installs `shelve` to `~/.x-cli-shelve/bin`, adds that directory to the
active shell profile when needed, and creates
`~/.config/shelve/config.toml` without replacing an existing config.

All `open` destinations come from `[[locations]]` in that config. The starter
config includes Home, Desktop, Downloads, and Documents; add any workspaces,
archives, projects, or other important folders there. `move_here = true` makes
a location available to `shelve move` as well.

The build output lives under `~/construction_side/shelve/target`.

## Development

```sh
make check
make run
```

## License

MIT

The picker separates groups with blank rows, indents folders below their group,
and keeps the selected folder and wrapped path in a padded detail pane. Narrow
terminals stack the panes; very small terminals show a resize hint.
