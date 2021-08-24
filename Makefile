build: 
	cargo build --all
fmt:
	cargo +nightly fmt
run: 
	RUST_LOG=trace ./target/debug/tproxy $(config)
test:
	cargo test --all
lint:
	cargo clippy --all-targets -- -D warnings