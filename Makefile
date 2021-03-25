build: 
	cargo build --package proxy --bin proxy
run: 
	sudo ./target/debug/proxy
set-env: 
	sh ./iptables.sh
clear-env: 
	sh ./iptables_clear.sh