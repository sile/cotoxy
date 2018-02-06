extern crate clap;
extern crate cotoxy;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate slog;
extern crate sloggers;
#[macro_use]
extern crate trackable;

use std::net::SocketAddr;
use clap::{App, Arg};
use cotoxy::Error;
use cotoxy::ProxyServerBuider;
use fibers::{Executor, Spawn};
use fibers::executor::InPlaceExecutor;
use sloggers::Build;
use sloggers::terminal::{Destination, TerminalLoggerBuilder};
use sloggers::types::SourceLocation;

macro_rules! try_parse {
    ($expr:expr) => { track_try_unwrap!($expr.parse().map_err(Error::from)) }
}

fn main() {
    let matches = App::new("cotoxy")
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("BIND_ADDR")
                .long("bind-addr")
                .takes_value(true)
                .default_value("0.0.0.0:17382"),
        )
        .arg(
            Arg::with_name("CONSUL_ADDR")
                .long("consul-addr")
                .takes_value(true)
                .default_value("127.0.0.1:8500"),
        )
        .arg(
            Arg::with_name("SERVICE")
                .short("s")
                .long("service")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("LOG_LEVEL")
                .long("log-level")
                .takes_value(true)
                .default_value("info")
                .possible_values(&["debug", "info", "warning", "error"]),
        )
        .get_matches();
    let bind_addr: SocketAddr = try_parse!(matches.value_of("BIND_ADDR").unwrap());
    let _consul_addr: SocketAddr = try_parse!(matches.value_of("CONSUL_ADDR").unwrap());
    let service = matches.value_of("SERVICE").unwrap().to_owned();
    let log_level = try_parse!(matches.value_of("LOG_LEVEL").unwrap());
    let logger = track_try_unwrap!(
        TerminalLoggerBuilder::new()
            .source_location(SourceLocation::None)
            .destination(Destination::Stderr)
            .level(log_level)
            .build()
    );

    let mut executor = InPlaceExecutor::new().unwrap();
    let logger = logger.new(o!("proxy" => bind_addr.to_string(), "service" => service.clone()));
    let proxy = ProxyServerBuider::new(&service)
        .logger(logger)
        .bind_addr(bind_addr)
        .finish(executor.handle());
    let fiber = executor.spawn_monitor(proxy);
    track_try_unwrap!(executor.run_fiber(fiber).unwrap().map_err(Error::from));
}
