.PHONY: all test

all: bin/dulook

bin/dulook: Cargo.toml Cargo.lock $(wildcard src/*.rs src/bin/*.rs)
	cargo build --release --locked --bin dulook
	install -Dm755 target/release/dulook bin/dulook

test:
	cargo test --locked
