build: 
	cargo build --package proxy --bin proxy
run: 
	sudo ./target/debug/tproxy
set-env: 
	sh ./iptables.sh
clear-env: 
	sh ./iptables_clear.sh