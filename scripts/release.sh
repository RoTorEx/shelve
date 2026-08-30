#!/usr/bin/env sh
set -eu

fail() { echo "ERROR: $*" >&2; exit 1; }
[ "$#" -eq 0 ] || fail "use: make release"
[ "$(git branch --show-current)" = main ] || fail "release must run from main"
[ -z "$(git status --porcelain)" ] || fail "commit changes before releasing"
git fetch origin main --tags
git merge-base --is-ancestor origin/main HEAD || fail "local main is behind or diverged from origin/main"
current="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
printf "Current version: %s\nRelease version (MAJOR.MINOR.PATCH): " "$current"
read -r version
printf '%s\n' "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || fail "invalid version"
tag="v$version"
! git rev-parse --verify "refs/tags/$tag" >/dev/null 2>&1 || fail "$tag already exists"
make check
temp="$(mktemp "${TMPDIR:-/tmp}/shelve-release.XXXXXX")"
trap 'rm -f "$temp"' EXIT HUP INT TERM
awk -v current="$current" -v version="$version" '
    !done && $0 == "version = \"" current "\"" {
        print "version = \"" version "\""
        done = 1
        next
    }
    { print }
    END { if (!done) exit 1 }
' Cargo.toml > "$temp" || fail "could not update Cargo.toml"
cp "$temp" Cargo.toml
cargo check >/dev/null
date_now="$(date +%Y-%m-%d)"
sed -i '' "/## \[Unreleased\]/a\\
\\
## [$version] - $date_now" CHANGELOG.md
make check
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "build: release $tag"
git tag -a "$tag" -m "Release $version"
trap - EXIT HUP INT TERM
rm -f "$temp"
echo "Prepared $tag"
