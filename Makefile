.PHONY: all test

# One prebuilt backend per architecture Omarchy runs on; bin/dulook starts the
# one matching `uname -m`. Cross-building needs zig, cargo-zigbuild, and
# `rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu`.
ARCHES := x86_64 aarch64
SOURCES := Cargo.toml Cargo.lock $(wildcard src/*.rs src/bin/*.rs)

all: $(ARCHES:%=bin/dulook-%)

bin/dulook-%: $(SOURCES)
	cargo zigbuild --release --locked --bin dulook --target $*-unknown-linux-gnu
	install -Dm755 target/$*-unknown-linux-gnu/release/dulook $@

test:
	cargo test --locked
