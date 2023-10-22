extern crate clap;
extern crate cotoxy;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate trackable;

use clap::Parser;
use cotoxy::Error;
use cotoxy::ProxyServerBuilder;
use fibers::executor::{InPlaceExecutor, ThreadPoolExecutor};
use fibers::{Executor, Spawn};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Parser)]
struct Args {
    /// Name of the service to which clients connect.
    service: String,

    /// TCP address to which the proxy bind.
    #[clap(long, default_value = "0.0.0.0:17382")]
    bind_addr: SocketAddr,

    /// TCP address of the consul agent which the proxy queries.
    #[clap(long, default_value = "127.0.0.1:8500")]
    consul_addr: SocketAddr,

    /// Port number of the service.
    #[clap(long)]
    service_port: Option<u16>,

    /// Datacenter to query.
    #[clap(long)]
    dc: Option<String>,

    /// Tag to filter service nodes on.
    #[clap(long)]
    tag: Option<String>,

    /// Node name to sort the service node list in ascending order
    /// based on the estimated round trip time from that node.
    /// If `_agent` is specified,
    /// the node of the consul agent being queried will be used for the sort.
    #[clap(long)]
    near: Option<String>,

    /// Node metadata key/value pair of the form `key:value`.
    /// Service nodes will be filtered with the specified key/value pairs.
    #[clap(long)]
    node_meta: Vec<String>,

    /// Number of worker threads.
    #[clap(long, default_value_t = 1)]
    threads: usize,

    /// TCP connect timeout in milliseconds.
    #[clap(long, default_value_t = 1000)]
    connect_timeout: u64,
}

fn main() {
    env_logger::init();

    let args = Args::parse();
    let bind_addr: SocketAddr = args.bind_addr;
    let consul_addr: SocketAddr = args.consul_addr;
    let service = args.service;
    let threads: usize = args.threads;
    let connect_timeout: u64 = args.connect_timeout;

    let mut proxy = ProxyServerBuilder::new(&service);
    proxy.bind_addr(bind_addr);
    proxy.connect_timeout(Duration::from_millis(connect_timeout));

    proxy.consul().consul_addr(consul_addr);
    if let Some(service_port) = args.service_port {
        proxy.service_port(service_port);
    }
    if let Some(dc) = args.dc {
        proxy.consul().dc(&dc);
    }
    if let Some(tag) = args.tag {
        proxy.consul().tag(&tag);
    }
    if let Some(near) = args.near {
        proxy.consul().near(&near);
    }
    for m in args.node_meta {
        let mut tokens = m.splitn(2, ':');
        let key = tokens.next().expect("Never fails");
        let value = tokens.next().unwrap_or("");
        proxy.consul().add_node_meta(key, value);
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
