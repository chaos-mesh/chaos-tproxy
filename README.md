# rs-tproxy

Transparent proxy in only linux 2.0+ platform for injecting [Abort|Delay|Replace] HTTP packet.



Based on linux iptables-tproxy method, the proxy do not need any L3|L4 config ( An valid local port is needed ). 

On the reason of safety ,  the proxy will only work for 30000+ port local process.

Config Example:

```
        // let c = TproxyConfig {
        //     port : 58080,
        //     mark:255,
        //     handler_config : HandlerConfig {
        //         packet: PacketTarget::Request,
        //         selector: Selector {
        //             path: Some(b"/rs-tproxy".to_vec()),
        //             method: None,
        //             code: None,
        //             header_fields: None
        //         },
        //         action: Action::Delay(tokio::time::Duration::from_millis(2000)),
        //     },
        // };
        // let c = TproxyConfig {
        //     port : 58080,
        //     mark:255,
        //     handler_config : HandlerConfig {
        //         packet: PacketTarget::Response,
        //         selector: Selector {
        //             path: Some(b"/rs-tproxy".to_vec()),
        //             method: None,
        //             code: None,
        //             header_fields: None
        //         },
        //         action: Action::Replace(b"HTTP/1.1 404\r\n\r\n".to_vec()),
        //     },
        // };
        let c = TproxyConfig {
            port : 58080,
            mark:255,
            handler_config : HandlerConfig {
                packet: PacketTarget::Response,
                selector: Selector {
                    path: Some(b"/rs-tproxy".to_vec()),
                    method: None,
                    code: None,
                    header_fields: None
                },
                action: Action::Abort,
            },
        };
```

`HandlerConfig`  is the http handler config :

`PacketTarget` maening the action target , select a request with [Method( like `Get` )|path( like `/proxy`)] on HTTP Request And than take action([Abort|Delay|Replace]) on its Response is valid.