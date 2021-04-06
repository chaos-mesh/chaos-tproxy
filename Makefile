build: 
	cargo build
fmt:
	cargo +nightly fmt
run: 
	RUST_LOG=trace ./target/debug/tproxy $(config)
test:
	cargo test
