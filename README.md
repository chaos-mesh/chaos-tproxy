# rs-tproxy

Transparent proxy in only linux 2.0+ platform for injecting [Abort|Delay|Replace] HTTP packet.



Based on linux iptables-tproxy method, the proxy do not need any L3|L4 config ( An valid local port is needed ). 

On the reason of safety ,  the proxy will only work for 30000+ port local process.

Usage Example:

```
proxy 0.1.0
The option of rs-proxy.

USAGE:
    proxy <input>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <input>    path of config file
```
Support json and yaml config. Example of config could be found in `./example/config`

#How to build:
```
make set-env & make build
```
 clear iptables config :

```
make clear-env
```
rust env is now needed