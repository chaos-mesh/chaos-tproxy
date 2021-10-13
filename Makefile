build: 
	cargo build --workspace
fmt:
	cargo fmt
run: 
	RUST_LOG=trace ./target/debug/tproxy $(config)
test: build
	cargo test -p rs-tproxy-proxy -p rs-tproxy-plugin -p rs-tproxy-controller 
lint:
	cargo clippy --all-targets -- -D warnings
clean:
	cargo clean

