# chaos-tproxy

Transparent HTTP proxy for [Abort|Delay|Append|Replace] packet.
Based on linux iptables-extension : TPROXY.

## Installation
### Kernel Modules

Check the installed kernel modules by `lsmod`, modules `ebtables`, `ebtable_broute` and `iptable_mangle` are required to make chaos-tproxy work.

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

- additions for container:

```
update-alternatives --set iptables /usr/sbin/iptables-legacy
```

### Install chaos-tproxy

Download chaos-tproxy in release.

### Tips for ubuntu user:
The DNS will be broken if you are using the dnsmasq server on 127.0.0.1:53.
Please change the default dns server to 8.8.8.8 or other global dns server by editting `/etc/resolv.conf` before using `chaos-tproxy`.

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
chaos-tproxy ./example.yaml -v
```
## Usage example: 

```
>chaos-tproxy -h

proxy 0.1.1
The option of proxy.

USAGE:
    chaos-tproxy [FLAGS] [OPTIONS] [FILE]

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
## Yaml config file example
```yaml
proxy_ports: [80] # option u16 vec ; Do nothing if not provided 
interface: eth33 # option string
rules: # option rule vec
  - target: Request # Request or Response. 
    # Stand for target packet to select & take actions.
    # If target is Response & selecting request info such as method or path , 
    # proxy will select request and take actions on Response.
    selector:
      path: /p* # option Match path of `Uri` with wildcard matches. Using [wildcard matches](https://www.wikiwand.com/en/Matching_wildcards).
#    abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
#                                           |--------|
#                                               |
#                                             path
      method: GET # option string
      # code: 200
      # request_headers: # option map<string ,string>
      #   A:B
      # response_headers: # option map<string ,string>
      #   a:b
    actions:
      abort: true # bool ; None is false
      delay: 1s # option Duration
      replace: # option RawReplaceAction
        body: # also support replace path , method ...
          update_content_length: false # true by default
          contents:
            type: TEXT
            value: '{"name": "Chaos Mesh", "message": "Hello!"}'
      patch: # option RawPatchAction
        queries:
          - [foo, bar]
          - [foo, other]
        body:
          update_content_length: false # true by default
          contents:
            type: JSON
            value: '{"message": "Hi!"}'
```



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
chaos-tproxy -v <configfilename>
```


### interactive mode

You can apply config by HTTP over stdio if interactive mode is enabled.

- apply

```bash
> chaos-tproxy -i
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