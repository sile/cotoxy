use std;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use futures::Future;
use miasht::Client as HttpClient;
use miasht::Method;
use miasht::builtin::headers::ContentLength;
use miasht::builtin::{FutureExt, IoExt};
use serde::de;
use serde::{Deserialize, Deserializer};
use serdeconv;
use trackable::error::{ErrorKindExt, Failed};

use {AsyncResult, Error};

#[derive(Debug, Clone)]
pub struct ConsulClientBuilder {
    agent_addr: SocketAddr,
    service: String,
}
impl ConsulClientBuilder {
    pub const DEFAULT_AGENT_ADDR: &'static str = "127.0.0.1:8500";

    pub fn new(service: &str) -> Self {
        ConsulClientBuilder {
            agent_addr: Self::DEFAULT_AGENT_ADDR.parse().expect("Never fails"),
            service: service.to_owned(),
        }
    }

    pub fn agent_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.agent_addr = addr;
        self
    }

    pub fn finish(&self) -> ConsulClient {
        ConsulClient {
            agent_addr: self.agent_addr,
            service: self.service.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ConsulClient {
    agent_addr: SocketAddr,
    service: String,
}
impl ConsulClient {
    pub fn new(agent_addr: SocketAddr) -> Self {
        ConsulClient {
            agent_addr,
            service: "foo".to_owned(),
        }
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
    pub fn find_candidates(&self) -> AsyncResult<Vec<ServiceNode>> {
        let future = http_get(
            self.agent_addr,
            format!("/v1/catalog/service/{}?near=_agent", self.service),
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

// https://www.consul.io/api/catalog.html#sample-response-3
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

    #[serde(rename = "TaggedAddresses")]
    pub tagged_addresses: TaggedAddresses,

    #[serde(rename = "NodeMeta")]
    pub node_meta: HashMap<String, String>,

    #[serde(rename = "CreateIndex")]
    pub create_index: u64,

    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,

    #[serde(rename = "ServiceAddress", deserialize_with = "deserialize_maybe_ipaddr")]
    pub service_address: Option<IpAddr>,

    #[serde(rename = "ServiceEnableTagOverride")]
    pub service_enable_tag_override: bool,

    #[serde(rename = "ServiceID")]
    pub service_id: String,

    #[serde(rename = "ServiceName")]
    pub service_name: String,

    #[serde(rename = "ServicePort")]
    pub service_port: u16, // TODO: option

    #[serde(rename = "ServiceTags")]
    pub service_tags: Vec<String>,
}
impl ServiceNode {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(
            self.service_address.unwrap_or(self.address),
            self.service_port,
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct TaggedAddresses {
    pub lan: IpAddr,
    pub wan: IpAddr,
}

fn deserialize_maybe_ipaddr<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<IpAddr>, D::Error>
where
    D: Deserializer<'de>,
{
    let addr = String::deserialize(deserializer)?;
    if addr.is_empty() {
        Ok(None)
    } else {
        let addr = addr.parse().map_err(|e| de::Error::custom(e))?;
        Ok(Some(addr))
    }
}
