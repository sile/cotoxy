use std::net::{IpAddr, SocketAddr};
use futures::Future;
use miasht::Client as HttpClient;
use miasht::Method;
use miasht::builtin::headers::ContentLength;
use miasht::builtin::{FutureExt, IoExt};
use serdeconv;
use trackable::error::{ErrorKindExt, Failed};

use {AsyncResult, Error};

#[derive(Debug)]
pub struct ConsulClient {
    agent_addr: SocketAddr,
}
impl ConsulClient {
    pub fn new(agent_addr: SocketAddr) -> Self {
        ConsulClient { agent_addr }
    }
    pub fn find_service_nodes(&self, service: &str) -> AsyncResult<Vec<ServiceNode>> {
        let future = http_get(
            self.agent_addr,
            format!("/v1/catalog/service/{}?near=_agent", service),
        ).and_then(|body| {
            track!(serdeconv::from_json_slice(&body).map_err(|e| Error::from(Failed.takes_over(e))))
        });
        Box::new(future)
    }
}

fn http_get(addr: SocketAddr, path: String) -> AsyncResult<Vec<u8>> {
    let future = HttpClient::new()
        .connect(addr)
        .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        .and_then(move |connection| {
            let mut req = connection.build_request(Method::Get, &path);
            req.add_raw_header("Host", b"localhost");
            req.add_header(&ContentLength(0));
            req.finish()
                .map_err(|e| track!(Error::from(Failed.takes_over(e))))
        })
        .and_then(|connection| {
            connection
                .read_response()
                .map_err(|e| track!(Error::from(Failed.takes_over(e))))
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

#[derive(Debug, Deserialize)]
pub struct ServiceNode {
    #[serde(rename = "ID")]
    pub id: String,

    #[serde(rename = "Node")]
    pub node: String,

    #[serde(rename = "Address")]
    pub address: IpAddr,

    #[serde(rename = "Datacenter")]
    pub datacenter: String,

    #[serde(rename = "ServicePort")]
    pub service_port: u16, // TODO: other field
}
impl ServiceNode {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.address, self.service_port)
    }
}
