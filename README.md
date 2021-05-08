# rs-tproxy

Transparent proxy in only linux 2.0+ platform for injecting [Abort|Delay|Append|Replace] HTTP packet.

Based on linux iptables-tproxy method, the proxy do not need any L3|L4 config ( An valid local port is needed ). 

You can run it as the proxy for any ports on the same network namespace by set the `proxy_ports` field in config file.

Usage Example:

```
proxy 0.1.0
The option of rs-proxy.

USAGE:
    tproxy [FLAGS] [FILE]

FLAGS:
    -h, --help           Prints help information
    -i, --interactive    Allows to apply config by stdin/stdout
    -V, --version        Prints version information
    -v, --verbose        Verbose mode (-v, -vv, -vvv, etc.)

ARGS:
    <FILE>    path of config file, required if interactive mode is disabled
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

### interactive mode

You can apply config by HTTP over stdio if interactive mode is enabled.

- apply

```bash
> sudo target/release/tproxy -i
PUT / HTTP/1.1
Content-Length: 129

{"proxy_ports": [30086], "rules": [{"target": "Request", "selector": {"path": "*", "method": "GET"},"actions": {"abort": true}}]}
```

- get the response:

```bash
HTTP/1.1 200 OK
content-length: 0
date: Mon, 03 May 2021 11:21:31 GMT
```

- recover

> Update config with empty proxy ports.

```
PUT / HTTP/1.1
Content-Length: 129

{"proxy_ports": [30086], "rules": [{"target": "Request", "selector": {"path": "*", "method": "GET"},"actions": {"abort": true}}]}
HTTP/1.1 200 OK
content-length: 0
date: Mon, 03 May 2021 11:21:31 GMT

PUT / HTTP/1.1
Content-Length: 124

{"proxy_ports": [], "rules": [{"target": "Request", "selector": {"path": "*", "method": "GET"},"actions": {"abort": true}}]}
HTTP/1.1 200 OK
content-length: 0
date: Mon, 03 May 2021 11:22:13 GMT
```

- exit

Ctrl-C