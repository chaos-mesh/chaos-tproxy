build: 
	cargo build --package proxy --bin proxy
run: 
	./target/debug/proxy $(config)
set-env: 
	sh ./iptables.sh
clear-env: 
	sh ./iptables_clear.sh
