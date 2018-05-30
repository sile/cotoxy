extern crate clap;
extern crate cotoxy;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate slog;
extern crate sloggers;
#[macro_use]
extern crate trackable;

use clap::{App, Arg};
use cotoxy::Error;
use cotoxy::ProxyServerBuilder;
use fibers::executor::{InPlaceExecutor, ThreadPoolExecutor};
use fibers::{Executor, Spawn};
use sloggers::terminal::{Destination, TerminalLoggerBuilder};
use sloggers::types::SourceLocation;
use sloggers::Build;
use std::net::SocketAddr;
use std::time::Duration;

const SERVICE_PORT_DEFAULT: &str = "<Port number registered in Consul>";
const DC_DEFAULT: &str = "<Datacenter of the consul agent being queried>";

macro_rules! try_parse {
    ($expr:expr) => {
        track_try_unwrap!($expr.parse().map_err(Error::from))
    };
}

fn main() {
    let matches = App::new("cotoxy")
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("SERVICE")
                .help("Name of the service to which clients connect")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("LOG_LEVEL")
                .long("log-level")
                .takes_value(true)
                .default_value("info")
                .possible_values(&["debug", "info", "warning", "error"]),
        )
        .arg(
            Arg::with_name("BIND_ADDR")
                .help("TCP address to which the proxy bind")
                .long("bind-addr")
                .takes_value(true)
                .default_value("0.0.0.0:17382"),
        )
        .arg(
            Arg::with_name("CONSUL_ADDR")
                .help("TCP address of the consul agent which the proxy queries")
                .long("consul-addr")
                .takes_value(true)
                .default_value("127.0.0.1:8500"),
        )
        .arg(
            Arg::with_name("SERVICE_PORT")
                .help("Port number of the service")
                .long("service-port")
                .takes_value(true)
                .default_value(SERVICE_PORT_DEFAULT),
        )
        .arg(
            Arg::with_name("DC")
                .help("Datacenter to query")
                .long("dc")
                .takes_value(true)
                .default_value(DC_DEFAULT),
        )
        .arg(
            Arg::with_name("TAG")
                .help("Tag to filter service nodes on")
                .long("tag")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("NEAR")
                .long_help(
                    "Node name to sort the service node list in ascending order \
                     based on the estimated round trip time from that node. \
                     If `_agent` is specified, \
                     the node of the consul agent being queried will be used for the sort.",
                )
                .long("near")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("NODE_META")
                .long_help(
                    "Node metadata key/value pair of the form `key:value`. \
                     Service nodes will be filtered with the specified key/value pairs.",
                )
                .long("node-meta")
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("THREADS")
                .help("Number of worker threads")
                .takes_value(true)
                .default_value("1"),
        )
        .arg(
            Arg::with_name("CONNECT_TIMEOUT")
                .help("TCP connect timeout in milliseconds")
                .long("connect-timeout")
                .takes_value(true)
                .default_value("1000"),
        )
        .get_matches();
    let bind_addr: SocketAddr = try_parse!(matches.value_of("BIND_ADDR").unwrap());
    let consul_addr: SocketAddr = try_parse!(matches.value_of("CONSUL_ADDR").unwrap());
    let service = matches.value_of("SERVICE").unwrap().to_owned();
    let threads: usize = try_parse!(matches.value_of("THREADS").unwrap());
    let connect_timeout: u64 = try_parse!(matches.value_of("CONNECT_TIMEOUT").unwrap());
    let log_level = try_parse!(matches.value_of("LOG_LEVEL").unwrap());
    let logger = track_try_unwrap!(
        TerminalLoggerBuilder::new()
            .source_location(SourceLocation::None)
            .destination(Destination::Stderr)
            .level(log_level)
            .build()
    );

    let logger = logger.new(o!("proxy" => bind_addr.to_string(), "service" => service.clone()));

    let mut proxy = ProxyServerBuilder::new(&service);
    proxy.logger(logger).bind_addr(bind_addr);
    proxy.connect_timeout(Duration::from_millis(connect_timeout));

    proxy.consul().consul_addr(consul_addr);
    if let Some(service_port) = matches.value_of("SERVICE_PORT") {
        if service_port != SERVICE_PORT_DEFAULT {
            let service_port: u16 = try_parse!(service_port);
            proxy.service_port(service_port);
        }
    }
    if let Some(dc) = matches.value_of("DC") {
        if dc != DC_DEFAULT {
            proxy.consul().dc(dc);
        }
    }
    if let Some(tag) = matches.value_of("TAG") {
        proxy.consul().tag(tag);
    }
    if let Some(near) = matches.value_of("NEAR") {
        proxy.consul().near(near);
    }
    if let Some(meta) = matches.values_of("NODE_META") {
        for m in meta {
            let mut tokens = m.splitn(2, ':');
            let key = tokens.next().expect("Never fails");
            let value = tokens.next().unwrap_or("");
            proxy.consul().add_node_meta(key, value);
        }
    }

    if threads == 1 {
        execute(InPlaceExecutor::new().unwrap(), &proxy);
    } else {
        execute(
            ThreadPoolExecutor::with_thread_count(threads).unwrap(),
            &proxy,
        );
    }
}

fn execute<E: Executor + Spawn>(mut executor: E, proxy: &ProxyServerBuilder) {
    let proxy = proxy.finish(executor.handle());
    let fiber = executor.spawn_monitor(proxy);
    track_try_unwrap!(executor.run_fiber(fiber).unwrap().map_err(Error::from));
}
