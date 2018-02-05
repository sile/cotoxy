extern crate clap;
extern crate cotoxy;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate trackable;

use std::net::SocketAddr;
use clap::{App, Arg};
use cotoxy::Error;
use cotoxy::proxy::Proxy;
use fibers::{Executor, Spawn};
use fibers::executor::InPlaceExecutor;
use fibers::net::TcpListener;
use futures::{Future, Stream};

fn main() {
    let matches = App::new("cotoxy")
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
        .get_matches();
    let bind_addr: SocketAddr = track_try_unwrap!(
        matches
            .value_of("BIND_ADDR")
            .unwrap()
            .parse()
            .map_err(Error::from_error)
    );
    let consul_addr: SocketAddr = track_try_unwrap!(
        matches
            .value_of("CONSUL_ADDR")
            .unwrap()
            .parse()
            .map_err(Error::from_error)
    );
    let service = matches.value_of("SERVICE").unwrap().to_owned();

    let mut executor = InPlaceExecutor::new().unwrap();

    let spawner = executor.handle();
    let fiber = executor.spawn_monitor(
        TcpListener::bind(bind_addr)
            .map_err(|e| track!(Error::from_error(e)))
            .and_then(move |listener| {
                println!("# Start listening: {:?}", listener.local_addr().ok());
                listener
                    .incoming()
                    .map_err(|e| track!(Error::from_error(e)))
                    .for_each(move |(client, _)| {
                        let service = service.clone();
                        spawner.spawn(client.map_err(|e| println!("# Error: {}", e)).and_then(
                            move |client| {
                                Proxy::new(client, consul_addr, service)
                                    .map_err(|e| println!("{}", track!(e)))
                            },
                        ));
                        Ok(())
                    })
            }),
    );
    track_try_unwrap!(
        executor
            .run_fiber(fiber)
            .unwrap()
            .map_err(Error::from_error)
    );
}
