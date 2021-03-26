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

## Build:
```
make build
```

## Usage

```bash
make run config=<path>
```

You must set iptables config after run:

```bash
make set-env 
```
Clear iptables config before stop:

```bash
make clear-env
```
