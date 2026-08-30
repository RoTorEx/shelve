#!/usr/bin/env sh
set -eu

repo="${SHELVE_INSTALL_REPO:-RoTorEx/shelve}"
version="${SHELVE_VERSION:-latest}"
install_dir="${SHELVE_INSTALL_DIR:-$HOME/.x-cli-shelve}"
config_dir="${SHELVE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/shelve}"
archive_path=""
update_path=1

usage() {
    cat <<'EOF'
Usage: shelve-install.sh [--version VERSION|latest] [--install-dir PATH] [--config-dir PATH] [--archive PATH] [--no-path-update]

Installs a macOS GitHub Release. Existing configuration is preserved.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version) version="${2:?missing version}"; shift 2 ;;
        --install-dir) install_dir="${2:?missing install directory}"; shift 2 ;;
        --config-dir) config_dir="${2:?missing config directory}"; shift 2 ;;
        --archive) archive_path="${2:?missing archive path}"; shift 2 ;;
        --no-path-update) update_path=0; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

[ "$(uname -s)" = "Darwin" ] || { echo "ERROR: shelve releases currently support macOS only" >&2; exit 1; }
case "$(uname -m)" in
    arm64|aarch64) arch=aarch64 ;;
    x86_64|amd64) arch=x86_64 ;;
    *) echo "ERROR: unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

archive="shelve-macos-$arch.tar.gz"
temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/shelve-install.XXXXXX")"
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM
token="${GH_INSTALLER_TOKEN:-${GH_TOKEN:-${GITHUB_TOKEN:-}}}"

download() {
    download_url=$1
    download_output=$2
    if [ -n "$token" ]; then
        printf 'header = "Authorization: Bearer %s"\n' "$token" |
            curl --config - -fsSL --output "$download_output" "$download_url"
    else
        curl -fsSL --output "$download_output" "$download_url"
    fi
}

if [ -n "$archive_path" ]; then
    cp "$archive_path" "$temp_dir/$archive"
else
    if [ "$version" = latest ]; then
        base="https://github.com/$repo/releases/latest/download"
    else
        base="https://github.com/$repo/releases/download/v${version#v}"
    fi
    download "$base/$archive" "$temp_dir/$archive"
    download "$base/$archive.sha256" "$temp_dir/$archive.sha256"
    expected="$(awk 'NR == 1 { print $1 }' "$temp_dir/$archive.sha256")"
    actual="$(shasum -a 256 "$temp_dir/$archive" | awk '{ print $1 }')"
    [ "$actual" = "$expected" ] || { echo "ERROR: archive checksum mismatch" >&2; exit 1; }
fi

tar -xzf "$temp_dir/$archive" -C "$temp_dir"
[ -x "$temp_dir/shelve" ] || { echo "ERROR: archive does not contain shelve" >&2; exit 1; }
[ -f "$temp_dir/config.example.toml" ] || { echo "ERROR: archive does not contain config.example.toml" >&2; exit 1; }

mkdir -p "$install_dir/bin" "$config_dir"
chmod 0700 "$install_dir" "$config_dir"
cp "$temp_dir/shelve" "$install_dir/bin/.shelve-install-$$"
chmod 0755 "$install_dir/bin/.shelve-install-$$"
mv "$install_dir/bin/.shelve-install-$$" "$install_dir/bin/shelve"

if [ ! -f "$config_dir/config.toml" ]; then
    cp "$temp_dir/config.example.toml" "$config_dir/config.toml"
    chmod 0600 "$config_dir/config.toml"
    echo "Created $config_dir/config.toml"
else
    echo "Preserved $config_dir/config.toml"
fi

if [ "$update_path" -eq 1 ]; then
    case "${SHELL:-}" in */bash) profile="$HOME/.bashrc" ;; *) profile="$HOME/.zshrc" ;; esac
    if [ "$install_dir" = "$HOME/.x-cli-shelve" ]; then
        line='export PATH="$HOME/.x-cli-shelve/bin:$PATH"'
    else
        line="export PATH=\"$install_dir/bin:\$PATH\""
    fi
    touch "$profile"
    grep -Fqx "$line" "$profile" || printf '\n# x-cli-shelve\n%s\n' "$line" >> "$profile"
fi

echo "Installed $install_dir/bin/shelve"
echo "Run: export PATH=\"$install_dir/bin:\$PATH\""
