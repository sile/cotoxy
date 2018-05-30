use futures::Future;
use miasht::builtin::headers::{Connection, ContentLength};
use miasht::builtin::{FutureExt, IoExt};
use miasht::Client as HttpClient;
use miasht::Method;
use std::net::SocketAddr;
use trackable::error::{ErrorKindExt, Failed};
use url::Url;

use {AsyncResult, Error};

pub fn get(addr: SocketAddr, url: Url) -> AsyncResult<Vec<u8>> {
    let mut path = url.path().to_owned();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }

    let future = HttpClient::new()
        .connect(addr)
        .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        .and_then(move |connection| {
            let mut req = connection.build_request(Method::Get, &path);
            if let Some(host) = url.host_str() {
                req.add_raw_header("Host", host.as_bytes());
            }
            req.add_header(&ContentLength(0));
            req.add_header(&Connection::Close);
            req.finish()
                .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        })
        .and_then(|connection| {
            connection
                .read_response()
                .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        })
        .and_then(|res| {
            let status = res.status().code();
            track_assert_eq!(status / 100, 2, Failed, "http_status:{}", status);
            Ok(res)
        })
        .and_then(|res| {
            res.into_body_reader()
                .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        })
        .and_then(|reader| {
            reader
                .read_all_bytes()
                .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        })
        .map(|(_, body)| body);
    Box::new(future)
}
