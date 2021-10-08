# rs-tproxy

Transparent HTTP proxy for [Abort|Delay|Append|Replace] packet.
Based on linux iptables-extension : TPROXY.

## Installation
### Install ebtables-legacy
Rs-tproxy relies on the legacy version of ebtables since the ebtables-nft have some problem on brouting transfer.
So on different linux distribution we need to install ebtables-legacy or create a symbolic link.
#### On centos 7.0 - 7.9 or other version using legacy version of ebtables:
```
ln -s /usr/sbin/ebtables /usr/sbin/ebtables-legacy
```
#### On centos 8+ :
```
wget http://mirror.coastal.edu/centos/8-stream/hyperscale/x86_64/packages-main/Packages/e/ebtables-legacy-2.0.11-9.hs.el8.x86_64.rpm
rpm -i ebtables-legacy-2.0.11-9.hs.el8.x86_64.rpm
rm ebtables-legacy-2.0.11-9.hs.el8.x86_64.rpm
yum reinstall iptables-ebtables
```
#### On Debian or Ubuntu :
```
apt install -y ebtables
```
### Install rs-tproxy

Download rs-tproxy in release.

## Quick start

```bash
cat > example.yaml<<EOF
proxy_ports: [80]
rules:
  - target: Request
    selector:
      method: GET
    actions:
      delay: 5s
EOF
rs-tproxy ./example.yaml -v
```
### Tips for ubuntu user:
The DNS will be broken if you are using the dnsmasq server on 127.0.0.1:53.
Please change the default dns server to 8.8.8.8 or other global dns server by editting `/etc/resolv.conf` after using `rs-tproxy`. 

## Usage example: 

```
>rs-tproxy -h

proxy 0.1.1
The option of proxy.

USAGE:
    rs-tproxy [FLAGS] [OPTIONS] [FILE]

FLAGS:
    -h, --help           Prints help information
    -i, --interactive    Allows applying json config by stdin/stdout
        --proxy          Only run the sub proxy
    -V, --version        Prints version information
    -v, --verbose        Verbose mode (-v, -vv, -vvv, etc.)

OPTIONS:
        --ipc-path <ipc-path>    ipc path for sub proxy

ARGS:
    <FILE>    path of config file, required if interactive and daemon mode is disabled
```
Support json and yaml config. 
Example of config could be found in `./config-examples`
todo: add config doc in https://github.com/chaos-mesh/website.


## Build:
```
make build
```

## Usage

```bash
make run config=<path>
```
or 
```
rs-tproxy <configfilename> -v
```


### interactive mode

You can apply config by HTTP over stdio if interactive mode is enabled.

- apply

```bash
> rs-tproxy -i
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