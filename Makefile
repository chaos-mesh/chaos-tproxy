build: 
	cargo build --all
fmt:
	cargo fmt
run: 
	RUST_LOG=trace ./target/debug/chaos-tproxy $(config)
test:
	cargo test --all
lint:
	cargo clippy --all-targets -- -D warnings
image:
	DOCKER_BUILDKIT=1 docker build \
		$(if $(HTTP_PROXY),--build-arg HTTP_PROXY) \
		$(if $(HTTPS_PROXY),--build-arg HTTPS_PROXY) \
		-t chaos-mesh/tproxy .
release: image
	docker run -v "$(CURDIR):/opt/mount:z" --rm --entrypoint cp chaos-mesh/tproxy /tproxy /opt/mount/tproxy
