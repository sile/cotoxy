cotoxy
======

[![cotoxy](http://meritbadge.herokuapp.com/cotoxy)](https://crates.io/crates/cotoxy)
[![Documentation](https://docs.rs/cotoxy/badge.svg)](https://docs.rs/cotoxy)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A TCP proxy using [Consul][consul] for service discovery.

This uses [List Nodes for Service] API for collecting candidate servers.

[consul]: https://www.consul.io/
[List Nodes for Service]: https://www.consul.io/api/catalog.html#list-nodes-for-service

Install
--------

### Precompiled binaries

A precompiled binary for Linux environment is available in the [releases] page.

```console
$ curl -L https://github.com/sile/cotoxy/releases/download/0.1.0/cotoxy-0.1.0.linux -o cotoxy
$ chmod +x cotoxy
$ ./cotoxy -h
cotoxy 0.1.0
A TCP proxy using Consul for service discovery

USAGE:
    cotoxy [OPTIONS] <SERVICE> [--] [THREADS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --bind-addr <BIND_ADDR>                TCP address to which the proxy bind [default: 0.0.0.0:17382]
        --connect-timeout <CONNECT_TIMEOUT>    TCP connect timeout in milliseconds [default: 1000]
        --consul-addr <CONSUL_ADDR>            TCP address of the consul agent which the proxy queries [default:
                                               127.0.0.1:8500]
        --dc <DC>                              Datacenter to query [default: <Datacenter of the consul agent being
                                               queried>]
        --log-level <LOG_LEVEL>                 [default: info]  [values: debug, info, warning, error]
        --near <NEAR>                          Node name to sort the service node list in ascending order based on the
                                               estimated round trip time from that node. If `_agent` is specified, the
                                               node of the consul agent being queried will be used for the sort.
        --node-meta <NODE_META>...             Node metadata key/value pair of the form `key:value`. Service nodes will
                                               be filtered with the specified key/value pairs.
        --service-port <SERVICE_PORT>          Port number of the service [default: <Port number registered in Consul>]
        --tag <TAG>                            Tag to filter service nodes on

ARGS:
    <SERVICE>    Name of the service to which clients connect
    <THREADS>    Number of worker threads [default: 1]
```

### Using Cargo

If you have already installed [Cargo][cargo], you can install `cotoxy` easily in the following command:

```console
$ cargo install cotoxy
```

[cargo]: https://doc.rust-lang.org/cargo/
[releases]: https://github.com/sile/cotoxy/releases

Examples
--------

```console
/// Run the consul agent in the background.
$ docker run -d --rm -p 8500:8500 consul

/// Start `cotoxy` which proxies "consul" service.
$ cotoxy consul --service-port 8500
Feb 07 13:23:13.028 INFO Proxy server started, service: consul, proxy: 0.0.0.0:17382

/// Call consul API via the proxy.
$ curl -s http://localhost:17382/v1/catalog/service/consul
[
    {
        "ID": "2db3224e-e9cb-fab4-edd7-6e98a2842f16",
        "Node": "e10993d941f1",
        "Address": "127.0.0.1",
        "Datacenter": "dc1",
        "TaggedAddresses": {
            "lan": "127.0.0.1",
            "wan": "127.0.0.1"
        },
        "NodeMeta": {
            "consul-network-segment": ""
        },
        "ServiceID": "consul",
        "ServiceName": "consul",
        "ServiceTags": [],
        "ServiceAddress": "",
        "ServicePort": 8300,
        "ServiceEnableTagOverride": false,
        "CreateIndex": 5,
        "ModifyIndex": 5
    }
]
```
