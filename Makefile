.PHONY: build test lint fmt check run

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

run:
	cargo run --locked --
