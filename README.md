# rs-tproxy

Transparent proxy in only linux 2.0+ platform for injecting [Abort|Delay|Append|Replace] HTTP packet.

Based on linux iptables-tproxy method, the proxy do not need any L3|L4 config ( An valid local port is needed ). 

You can run it as the proxy for any ports on the same network namespace by set the `proxy_ports` field in config file.

Usage Example:

```
proxy 0.1.0
The option of tproxy.

USAGE:
    tproxy <input>

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
