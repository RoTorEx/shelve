# shelve

A tiny macOS CLI for opening important folders and shelving PDFs:

```text
shelve open [SELECTOR]
shelve move [FILE_OR_DIRECTORY ...]
shelve update
```

Running `shelve` without arguments opens the folder menu. `open` chooses a configured folder and opens it in Finder. `move` finds PDFs,
asks for a destination for each file, prints one batch preview, and moves only
after confirmation. Existing files are never overwritten.

Check the installed version with `shelve -V`. `shelve update` downloads the
latest compatible macOS release, verifies its checksum, and replaces the
current binary.

The folder menu follows Hop: lettered groups (`A`, `B`, …) and numbered
folders (`1`, `2`, …), with blank lines between groups and indented entries.
Type `B2` and press Enter to choose a folder. Codes are case-insensitive.
Empty input or `q` followed by Enter cancels. The menu stays in terminal
scrollback, so you can scroll to review all groups.

Use `shelve open B2` to open a known destination directly. Codes follow config
order and remain the same in `open` and `move`; PDF selection shows only folders
with `move_here = true`. Editing config order can change codes. Color follows
Hop's group/number/folder hierarchy and is disabled for redirected output,
`NO_COLOR`, or a dumb terminal.

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

Group headers and folder names come from actual path components, never custom
labels. Headers show the root name followed by its parent path, matching Hop. `<letter>0` opens that root without
adding a numbered root entry; folders start at 1. Root shortcuts are open-only
and cannot be used to bypass `move_here` restrictions. Relative path context is
shown only when a folder is deeper than the group root.

The menu follows Hop’s spacing and color roles, with a dim version and dividers,
a parent path in each sector heading, and a single short input prompt. Usage
help stays in documentation rather than appearing below every menu.
