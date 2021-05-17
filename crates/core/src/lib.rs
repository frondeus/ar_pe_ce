pub use anyhow::{Error, Result};
pub use async_trait::async_trait;

#[doc(hidden)]
pub mod re {
    pub use anyhow::Context;
    pub use bytes::Bytes;
    pub use futures::{future::ready, stream::once, TryStreamExt};
    pub use hyper::{
        service::{make_service_fn, service_fn},
        Body, Request, Response, Server, StatusCode,
    };
    pub use tracing::instrument;
    pub use url::Url;
}

use anyhow::{bail, Context};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::stream::{StreamExt, TryStreamExt};
use hyper::{client::connect::HttpConnector, Body, Client};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::pin::Pin;
use url::Url;

pub type Stream<T> = Pin<Box<dyn futures::Stream<Item = Result<T>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DecodeState {
    ReadHeader,
    ReadBody { len: usize },
}

fn decode_chunk<T>(buf: &mut BytesMut, state: &mut DecodeState) -> Result<Option<T>>
where
    T: DeserializeOwned + Send,
{
    if *state == DecodeState::ReadHeader {
        if buf.remaining() < 4 {
            return Ok(None);
        }

        let len = buf.get_u32() as usize;
        buf.reserve(len);
        *state = DecodeState::ReadBody { len };
    }

    if let DecodeState::ReadBody { len } = *state {
        if buf.remaining() < len || buf.len() < len {
            return Ok(None);
        }

        let to_decode = buf.split_to(len).freeze();

        let msg = rmp_serde::from_read_ref(&to_decode).context("Could not deserialize message")?;

        *state = DecodeState::ReadHeader;
        return Ok(Some(msg));
    }

    Ok(None)
}

pub fn decode_stream<T>(
    stream: impl futures::Stream<Item = Result<Bytes, hyper::Error>> + Send + 'static,
) -> impl futures::Stream<Item = Result<T>> + Send
where
    T: DeserializeOwned + Send,
{
    let mut buf = BytesMut::with_capacity(8 * 1024);
    let mut state = DecodeState::ReadHeader;

    async_stream::stream! {
        futures::pin_mut!(stream);

        loop {
            match decode_chunk(&mut buf, &mut state) {
                Err(e) => {
                    yield Err(e);
                    continue;
                },
                Ok(Some(t)) => {
                    yield Ok(t);
                    continue;
                }
                Ok(None) => ()
            }

            match stream.next().await {
                Some(Ok(bytes)) => {

                    buf.put(bytes);



                },
                Some(Err(e)) => yield Err(Error::from(e)),
                None => break,
            }
        }
    }
}

pub fn encode_stream<T>(
    stream: impl futures::Stream<Item = Result<T>> + Send + 'static,
) -> impl futures::Stream<Item = Result<Bytes>> + Send + 'static
where
    T: Serialize,
{
    let mut buf = BytesMut::with_capacity(8 * 1024).writer();
    stream.and_then(move |msg| {
        futures::future::ready({
            buf.get_mut().reserve(4);
            unsafe {
                buf.get_mut().advance_mut(4);
            }

            if let Err(e) =
                rmp_serde::encode::write(&mut buf, &msg).context("Could not serialize message")
            {
                return futures::future::ready(Result::Err(e));
            }

            let len = buf.get_ref().len() - 4;
            assert!(len <= std::u32::MAX as usize);
            {
                let buf = buf.get_mut();
                let mut buf = &mut buf[..4];
                buf.put_u32(len as u32);
            }

            let encoded = buf.get_mut().split_to(len + 4).freeze();
            Ok(encoded)
        })
    })
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
