#!/usr/bin/env sh
set -eu

binary="${1:?binary path is required}"
install_dir="${2:?install directory is required}"
config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/shelve"

[ -x "$binary" ] || { echo "ERROR: missing binary $binary" >&2; exit 1; }
mkdir -p "$install_dir/bin" "$config_dir"
chmod 0700 "$install_dir"
temporary="$install_dir/bin/.shelve-install-$$"
trap 'rm -f "$temporary"' EXIT HUP INT TERM
cp "$binary" "$temporary"
chmod 0755 "$temporary"
mv "$temporary" "$install_dir/bin/shelve"
trap - EXIT HUP INT TERM

if [ ! -f "$config_dir/config.toml" ]; then
    cp config.example.toml "$config_dir/config.toml"
    echo "Created $config_dir/config.toml"
else
    echo "Preserved $config_dir/config.toml"
fi

case "${SHELL:-}" in */bash) profile="$HOME/.bashrc" ;; *) profile="$HOME/.zshrc" ;; esac
line='export PATH="$HOME/.x-cli-shelve/bin:$PATH"'
touch "$profile"
grep -Fqx "$line" "$profile" || printf '\n# x-cli-shelve\n%s\n' "$line" >> "$profile"

echo "Installed $install_dir/bin/shelve"
echo "For this shell: export PATH=\"$install_dir/bin:\$PATH\""
