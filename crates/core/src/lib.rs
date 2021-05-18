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

pub type Stream<T> = std::pin::Pin<Box<dyn futures::Stream<Item = Result<T>> + Send + 'static>>;

mod client;
mod encoding;
mod server {
    use crate::Result;
    use async_trait::async_trait;

    //TODO:
    #[async_trait]
    pub trait Rpc<T, O> {
        async fn call(&self, req: T) -> Result<O>;
    }
}

pub use client::*;
pub use encoding::*;
pub use server::*;
