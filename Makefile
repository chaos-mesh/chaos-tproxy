build: 
	cargo build
run: 
	sudo ./target/debug/rs-tproxy
set-env: 
	sh ./iptables.sh
clear-env: 
	sh ./iptables_clear.sh