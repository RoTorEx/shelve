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

The current config format uses sections with explicit roots:

```toml
version = 2
inboxes = ["~/Desktop", "~/Downloads"]

[[sections]]
root = "~/Documents/WorkSpace/Library"
children = true
pins = [
  "Books",
  "Guides/English",
]
```

`<sector>0` always opens the configured `root`. Relative pins are resolved from
that root. Absolute and home-relative pins are accepted only when they still
resolve below the root; an umbrella cannot contain unrelated folders. Pins
appear first in config order. `children = true` appends the root's other
immediate, non-hidden directories alphabetically; Shelve does not recursively
flood the menu. Duplicate paths are shown once. Omit both `pins` and `children`
when a section should expose only its root via `<sector>0`.

Use `move_here` as an explicit allowlist. Every listed path must also be a pin
or an automatically discovered child:

```toml
[[sections]]
root = "~/Documents/WorkSpace/Business/PL JDG"
children = true
pins = ["In Invoices", "Out Invoices"]
move_here = ["In Invoices", "Out Invoices"]
```

Legacy `[[locations]]` configs remain supported. In that format,
`move_here = true` on an individual location retains its existing meaning.

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
`move_here` restrictions. A deeper pin shows its parent relative to the root;
every pin remains under its section root.

The menu follows Hop’s spacing and color roles, with a dim version and dividers,
a parent path in each sector heading, and a single short input prompt. Usage
help stays in documentation rather than appearing below every menu.
