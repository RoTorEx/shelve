# shelve

A tiny macOS CLI for opening important folders and shelving PDFs:

```text
shelve open [SELECTOR]
shelve move [FILE_OR_DIRECTORY ...]
shelve update
```

Running `shelve` without arguments shows help. `shelve open` chooses a configured folder and opens it in Finder. `move` lists candidate PDFs and first asks which files to move (for example
`A1 A3`). Nothing is preselected, including when paths are passed explicitly.
It then asks for a destination for each selected file, prints one batch preview, and moves only
after confirmation. Existing files are never overwritten.

Check the installed version with `shelve -V`. `shelve update` downloads the
latest compatible macOS release, verifies its checksum, and replaces the
current binary.

The folder menu follows Hop: lettered groups (`A`, `B`, …) and numbered
folders (`1`, `2`, …), with blank lines between groups and indented entries.
Type `B2` and press Enter to choose a folder. Codes are case-insensitive.
Empty input or `q` followed by Enter cancels. The menu stays in terminal
scrollback, so you can scroll to review all groups.

Use `shelve open B2` to open a known destination directly. Codes follow section
and folder order and remain the same in `open` and `move`; every displayed
folder is available to both commands. Editing the config or folder tree can
change codes. Color follows Hop's group/number/folder hierarchy and is disabled
for redirected output, `NO_COLOR`, or a dumb terminal.

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

The current config format uses sections with explicit roots:

```toml
version = 2
inboxes = ["~/Desktop", "~/Downloads"]

[[sections]]
root = "~/Documents/WorkSpace/Library"
items = [
  "Books",
  "Guides",
  "Images",
  "Guides/English",
]
```

`<sector>0` always opens the configured `root`. Every section requires `items`;
an empty list exposes only the root. Items are resolved relative to the root,
may name descendants at any depth, appear strictly in config order, and are
deduplicated. Absolute and home-relative items are accepted only when they
still resolve below the root, so an umbrella cannot contain unrelated folders.
Nothing is discovered or added automatically. Every displayed item is
available to both `open` and `move`.

Legacy `[[locations]]` configs remain readable, but `move_here` is no longer
needed or used.

The build output lives under `~/construction_side/shelve/target`.

## Development

```sh
make check
make run
```

## License

MIT

Section headers and folder names come from actual path components, never custom
labels. Headers show the explicit root name followed by its parent path,
matching Hop. `<letter>0` opens that root without adding a numbered root entry;
folders start at 1. Root shortcuts are open-only and cannot be used to bypass
the numbered folder list. A deeper item shows its parent relative to the root;
every item remains under its section root.

The menu follows Hop’s spacing and color roles, with a dim version and dividers,
a parent path in each sector heading, and a single short input prompt. Usage
help stays in documentation rather than appearing below every menu.
