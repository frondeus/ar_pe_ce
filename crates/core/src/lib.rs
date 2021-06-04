pub use async_trait::async_trait;

#[doc(hidden)]
pub mod re {
    pub use crate::*;

    pub use anyhow::{self, Context};
    pub use bytes::{Bytes, BytesMut};
    pub use futures::{
        future::ready, pin_mut, stream::empty, stream::once, SinkExt, StreamExt, TryStreamExt,
    };
    pub use serde::{Deserialize, Serialize};
    pub use tokio::net::TcpStream;
    pub use tokio_util::codec::{FramedRead, FramedWrite};
    pub use tracing::instrument;
}

pub type Stream<T, E = Infallible> =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<T, E>> + Send + 'static>>;

mod client;
mod encoding;
mod error;
mod network;
mod server;

pub use client::*;
pub use encoding::*;
pub use error::*;
pub use network::*;
pub use server::*;
