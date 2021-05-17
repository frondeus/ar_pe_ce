use anyhow::{bail, Context};
use hyper::{client::connect::HttpConnector, Body, Client};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use url::Url;
use crate::Result;
use crate::{encode_stream, decode_stream};

pub struct ClientInner {
    client: Client<HttpConnector, Body>,
    url: Url,
}

impl ClientInner {
    pub fn new(url: Url) -> Self {
        let client: Client<HttpConnector, Body> = Client::builder().http2_only(true).build_http();

        Self { client, url }
    }

    pub async fn get<T, O>(
        &self,
        method: &'static str,
        args: impl futures::Stream<Item = Result<O>> + Send + 'static,
    ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = Result<T>> + Send>>>
    where
        T: DeserializeOwned + Send + 'static,
        O: Serialize + 'static,
    {
        let uri: String = self.url.join(method).context("Creating valid URL")?.into();
        let uri: hyper::Uri = uri.parse().context("Parsing URI from URL")?;

        let args = encode_stream(args);

        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .body(Body::wrap_stream(args))
            .context("Could not create request body")?;

        let resp = self.client.request(req).await.context("Sending request")?;
        let status = resp.status();
        tracing::info!(status = %status);

        if status.is_client_error() {
            bail!("Received status: {}", status);
        }

        if status.is_server_error() {
            bail!("Received status: {}", status);
        }

        let stream = decode_stream(resp.into_body());

        Ok(Box::pin(stream))
    }
}
