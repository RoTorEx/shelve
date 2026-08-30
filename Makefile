.PHONY: install-local build test lint fmt check run package-release smoke-install release release-push vibe-kernel-path vibe-kernel-set vibe-pull vibe-propose

BIN_NAME := shelve
CONSTRUCTION_SIDE ?= $(HOME)/construction_side
CARGO_TARGET_DIR ?= $(CONSTRUCTION_SIDE)/shelve/target
INSTALL_DIR ?= $(HOME)/.x-cli-shelve
DIST_DIR ?= $(CONSTRUCTION_SIDE)/shelve/dist
TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
export CARGO_TARGET_DIR

build:
	cargo build --release --locked

test:
	cargo test --locked

lint:
	cargo clippy --locked --all-targets -- -D warnings

fmt:
	cargo fmt --all

check:
	cargo fmt --all -- --check
	cargo check --locked --all-targets
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked --all-targets
	cargo build --release --locked
	sh -n scripts/*.sh
	sh scripts/install.sh --help >/dev/null

run:
	cargo run --locked --

install-local: build
	sh scripts/install-local.sh "$(CARGO_TARGET_DIR)/release/$(BIN_NAME)" "$(INSTALL_DIR)"

package-release: build
	sh scripts/package-release.sh "$(TARGET)" "$(VERSION)" "$(DIST_DIR)"

smoke-install:
	sh scripts/smoke-install.sh "$(TARGET)" "$(VERSION)" "$(DIST_DIR)"

release:
	sh scripts/release.sh

release-push:
	@set -eu; \
	branch="$$(git branch --show-current)"; \
	test "$$branch" = "main" || { echo "ERROR: releases must be pushed from main" >&2; exit 1; }; \
	version="$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"; \
	git rev-parse -q --verify "refs/tags/v$$version" >/dev/null || { echo "ERROR: missing v$$version" >&2; exit 1; }; \
	git push origin main --follow-tags

vibe-kernel-path:
	@test -f .vibe/KERNEL_SOURCE || { echo "Missing .vibe/KERNEL_SOURCE. Run: make vibe-kernel-set" >&2; exit 1; }
	@sed -n '1p' .vibe/KERNEL_SOURCE

vibe-kernel-set:
	@mkdir -p .vibe; \
	if [ -n "$(KERNEL)" ]; then kernel_root="$(KERNEL)"; else printf "Kernel path: "; read -r kernel_root; fi; \
	case "$$kernel_root" in /*) ;; *) echo "ERROR: kernel path must be absolute." >&2; exit 1;; esac; \
	test -f "$$kernel_root/tools/vibe-pull" || { echo "ERROR: invalid kernel path: $$kernel_root" >&2; exit 1; }; \
	printf "%s\n" "$$kernel_root" > .vibe/KERNEL_SOURCE

vibe-pull:
	@test -f .vibe/KERNEL_SOURCE || { echo "Missing .vibe/KERNEL_SOURCE. Run: make vibe-kernel-set" >&2; exit 1; }
	@kernel_root="$$(sed -n '1p' .vibe/KERNEL_SOURCE)"; \
	python3 "$$kernel_root/tools/vibe-pull" .

# VIBE:KERNEL_MAKE_START

.PHONY: vibe-propose

vibe-propose:
	@test -f .vibe/KERNEL_SOURCE || { echo "Missing .vibe/KERNEL_SOURCE. Run: make vibe-kernel-set" >&2; exit 1; }
	@kernel_root="$$(sed -n '1p' .vibe/KERNEL_SOURCE)"; \
	test -f "$$kernel_root/tools/vibe-propose" || { echo "Missing $$kernel_root/tools/vibe-propose. Update the kernel source first." >&2; exit 1; }; \
	python3 "$$kernel_root/tools/vibe-propose" .

# VIBE:KERNEL_MAKE_END
