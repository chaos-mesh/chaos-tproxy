build: 
	cargo build
image:
	docker build -t tproxy .
fmt:
	cargo +nightly fmt
run: 
	./target/debug/tproxy $(config)
test:
	cargo test --all
set-env: 
	sh ./iptables.sh
clear-env: 
	sh ./iptables_clear.sh
