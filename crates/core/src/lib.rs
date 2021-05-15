pub use anyhow::{Error, Result};
pub use async_trait::async_trait;

pub use anyhow;
pub use futures;
pub use hyper;
pub use tracing;
pub use url;

use futures::{future, stream::TryStreamExt};
use hyper::{client::connect::HttpConnector, Body, Client};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use url::Url;

pub type Stream<T> = Pin<Box<dyn futures::Stream<Item = Result<T>> + Send + Sync + 'static>>;

pub struct ClientInner {
    client: Client<HttpConnector, Body>,
    url: Url,
}

impl ClientInner {
    pub fn new(url: Url) -> Self {
        let client: Client<HttpConnector, Body> = Client::builder().http2_only(true).build_http();

        Self { client, url }
    }

    pub async fn get_empty<T: DeserializeOwned + Send>(
        &self,
        method: &'static str,
    ) -> anyhow::Result<impl futures::Stream<Item = Result<T>> + Send> {
        let uri: String = self.url.join(method)?.into();
        let uri = uri.parse()?;

        let resp = self.client.get(uri).await?;
        let status = resp.status();
        tracing::info!(status = %status);

        if status.is_client_error() {
            anyhow::bail!("Received status: {}", status);
        }

        if status.is_server_error() {
            anyhow::bail!("Received status: {}", status);
        }

        let stream = resp
            .into_body()
            .try_filter(|i| future::ready(!i.is_empty()))
            .map_err(Error::from)
            .and_then(|it| future::ready(rmp_serde::from_read_ref(&it).map_err(Error::from)));

        Ok(stream)
    }

    pub async fn get<T: DeserializeOwned + Send, O: Serialize>(
        &self,
        method: &'static str,
        args: impl futures::Stream<Item = Result<O>> + Send + 'static,
    ) -> anyhow::Result<impl futures::Stream<Item = Result<T>> + Send> {
        let uri: String = self.url.join(method)?.into();
        let uri: hyper::Uri = uri.parse()?;

        let args = args
            .and_then(|res| futures::future::ready(rmp_serde::to_vec(&res).map_err(Error::from)));

        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .body(Body::wrap_stream(args))?;

        let resp = self.client.request(req).await?;
        let status = resp.status();
        tracing::info!(status = %status);

        if status.is_client_error() {
            anyhow::bail!("Received status: {}", status);
        }

        if status.is_server_error() {
            anyhow::bail!("Received status: {}", status);
        }

        let stream = resp
            .into_body()
            .try_filter(|i| future::ready(!i.is_empty()))
            .map_err(Error::from)
            .and_then(|it| future::ready(rmp_serde::from_read_ref(&it).map_err(Error::from)));

        Ok(stream)
    }
}
