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
image:
	DOCKER_BUILDKIT=1 docker build --build-arg HTTP_PROXY --build-arg HTTPS_PROXY . -t chaos-mesh/tproxy
release: image
	docker run -v ${PWD}:/opt/mount:z --rm --entrypoint cp chaos-mesh/tproxy /tproxy /opt/mount/tproxy