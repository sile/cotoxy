use std;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use futures::Future;
use serde::de;
use serde::{Deserialize, Deserializer};
use serdeconv;
use trackable::error::{ErrorKindExt, Failed};
use url::Url;

use {AsyncResult, Error};
use http;

/// Settings for Consul.
#[derive(Debug, Clone)]
pub struct ConsulSettings {
    consul_addr: SocketAddr,
    service: String,
    dc: Option<String>,
    tag: Option<String>,
    near: Option<String>,
    node_meta: Vec<(String, String)>,
}
impl ConsulSettings {
    /// The default consul agent address.
    pub const DEFAULT_CONSUL_ADDR: &'static str = "127.0.0.1:8500";

    /// Makes a new `ConsulSettings` instance.
    pub fn new(service: &str) -> Self {
        ConsulSettings {
            consul_addr: Self::DEFAULT_CONSUL_ADDR.parse().expect("Never fails"),
            service: service.to_owned(),
            dc: None,
            tag: None,
            near: None,
            node_meta: Vec::new(),
        }
    }

    /// Sets the address of the consul agent used by `ProxyServer`.
    ///
    /// The default value is `ConsulSettings::DEFAULT_CONSUL_ADDR`.
    pub fn consul_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.consul_addr = addr;
        self
    }

    /// Sets the value of the `dc` query parameter of [List Nodes for Service] API.
    ///
    /// [List Nodes for Service]: https://www.consul.io/api/catalog.html#list-nodes-for-service
    pub fn dc(&mut self, dc: &str) -> &mut Self {
        self.dc = Some(dc.to_owned());
        self
    }

    /// Sets the value of the `tag` query parameter of [List Nodes for Service] API.
    ///
    /// [List Nodes for Service]: https://www.consul.io/api/catalog.html#list-nodes-for-service.
    pub fn tag(&mut self, tag: &str) -> &mut Self {
        self.tag = Some(tag.to_owned());
        self
    }

    /// Sets the value of the `near` query parameter of [List Nodes for Service] API.
    ///
    /// [List Nodes for Service]: https://www.consul.io/api/catalog.html#list-nodes-for-service.
    pub fn near(&mut self, near: &str) -> &mut Self {
        self.near = Some(near.to_owned());
        self
    }

    /// Adds an entry for the `node-meta` query parameter of [List Nodes for Service] API.
    ///
    /// [List Nodes for Service]: https://www.consul.io/api/catalog.html#list-nodes-for-service.
    pub fn add_node_meta(&mut self, key: &str, value: &str) -> &mut Self {
        self.node_meta.push((key.to_owned(), value.to_owned()));
        self
    }

    pub(crate) fn client(&self) -> ConsulClient {
        ConsulClient {
            consul_addr: self.consul_addr,
            query_url: self.build_query_url(),
        }
    }

    fn build_query_url(&self) -> Url {
        let mut url = Url::parse(&format!("http://{}/v1/catalog/service", self.consul_addr))
            .expect("Never fails");
        url.path_segments_mut()
            .expect("Never fails")
            .push(&self.service);
        if let Some(ref dc) = self.dc {
            url.query_pairs_mut().append_pair("dc", dc);
        }
        if let Some(ref tag) = self.tag {
            url.query_pairs_mut().append_pair("tag", tag);
        }
        if let Some(ref near) = self.near {
            url.query_pairs_mut().append_pair("near", near);
        }
        for &(ref k, ref v) in &self.node_meta {
            url.query_pairs_mut()
                .append_pair("node_meta", &format!("{}:{}", k, v));
        }
        url
    }
}

#[derive(Debug)]
pub struct ConsulClient {
    consul_addr: SocketAddr,
    query_url: Url,
}
impl ConsulClient {
    pub fn find_candidates(&self) -> AsyncResult<Vec<ServiceNode>> {
        let future = http::get(self.consul_addr, self.query_url.clone()).and_then(|body| {
            track!(serdeconv::from_json_slice(&body).map_err(|e| Error::from(Failed.takes_over(e))))
        });
        Box::new(future)
    }

    pub fn query_url(&self) -> &Url {
        &self.query_url
    }
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
    pub fn socket_addr(&self, port: Option<u16>) -> SocketAddr {
        SocketAddr::new(
            self.service_address.unwrap_or(self.address),
            port.unwrap_or(self.service_port),
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
        let addr = addr.parse().map_err(de::Error::custom)?;
        Ok(Some(addr))
    }
}
