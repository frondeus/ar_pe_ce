pub use anyhow::{Error, Result};
pub use async_trait::async_trait;

#[doc(hidden)]
pub mod re {
    pub use anyhow::Context;
    pub use futures::{future::ready, stream::once, TryStreamExt};
    pub use hyper::{
        service::{make_service_fn, service_fn},
        Body, Request, Response, Server, StatusCode,
    };
    pub use tracing::instrument;
    pub use url::Url;
}

use futures::{future, stream::TryStreamExt};
use hyper::{client::connect::HttpConnector, Body, Client};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use url::Url;
use anyhow::{Context, bail};

pub type Stream<T> = Pin<Box<dyn futures::Stream<Item = Result<T>> + Send + Sync + 'static>>;

pub fn deserialize<'a, R, T>(rd: &'a R) -> Result<T>
where
    R: AsRef<[u8]> + std::fmt::Debug,
    T: serde::de::Deserialize<'a> {
    dbg!(&rd);
    rmp_serde::from_read_ref(rd)
        .context("Could not deserialize message")
        .map_err(Error::from)
}

pub fn serialize<T>(val: &T) -> Result<Vec<u8>>
where
    T: Serialize {
    rmp_serde::to_vec(val)
        .context("Could not serialize message")
        .map_err(Error::from)
}

pub struct ClientInner {
    client: Client<HttpConnector, Body>,
    url: Url,
}

impl ClientInner {
    pub fn new(url: Url) -> Self {
        let client: Client<HttpConnector, Body> = Client::builder().http2_only(true).build_http();

        Self { client, url }
    }

    pub async fn get<T: DeserializeOwned + Send, O: Serialize>(
        &self,
        method: &'static str,
        args: impl futures::Stream<Item = Result<O>> + Send + 'static,
    ) -> anyhow::Result<impl futures::Stream<Item = Result<T>> + Send> {
        let uri: String = self.url.join(method).context("Creating valid URL")?.into();
        let uri: hyper::Uri = uri.parse().context("Parsing URI from URL")?;

        let args = args
            .and_then(|res| futures::future::ready(rmp_serde::to_vec(&res).map_err(Error::from)));

        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .body(Body::wrap_stream(args)).context("Could not create request body")?;

        let resp = self.client.request(req).await.context("Sending request")?;
        let status = resp.status();
        tracing::info!(status = %status);

        if status.is_client_error() {
            bail!("Received status: {}", status);
        }

        if status.is_server_error() {
            bail!("Received status: {}", status);
        }

        let stream = resp
            .into_body()
            .try_filter(|i| future::ready(!i.is_empty()))
            .map_err(Error::from)
            .and_then(|it| future::ready(deserialize(&it)));

        Ok(stream)
    }
}
