use std::{fmt::Display, marker::PhantomData};

use futures::{future::ready, SinkExt, StreamExt, TryStreamExt};
use serde::{de::DeserializeOwned, Serialize};
use tokio::net::ToSocketAddrs;
use tokio::net::{tcp::OwnedReadHalf, TcpStream};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{Error, Headers, Result, Stream};

pub struct ClientInner<A: Send + Sync, M> {
    addr: A,
    _phantom: PhantomData<M>,
}

use std::fmt::Debug;

impl<A, M> ClientInner<A, M>
where
    A: ToSocketAddrs + Send + Sync,
    M: Serialize + std::fmt::Debug,
{
    pub fn new(addr: A) -> Self {
        Self {
            addr,
            _phantom: Default::default(),
        }
    }

    async fn get_inner<I, EC>(
        &self,
        headers: Headers<M>,
        input_stream: Stream<I, EC>,
    ) -> anyhow::Result<OwnedReadHalf>
    where
        I: Serialize + Send + 'static + Debug,
        EC: Debug + Display + std::error::Error + Send + Sync + 'static + Serialize,
    {
        let socket = TcpStream::connect(&self.addr).await?;
        let (stream, sink) = socket.into_split();

        let mut framed = FramedWrite::new(sink, crate::DefaultCodec::<Headers<M>>::default());
        framed.send(headers).await?;
        let sink = framed.into_inner();

        tokio::spawn(async move {
            let mut input_stream = input_stream.map(Ok);
            let mut framed =
                FramedWrite::new(sink, crate::DefaultCodec::<Result<I, EC>>::default());
            if let Err(err) = framed.send_all(&mut input_stream).await {
                tracing::error!(?err, "Could not send frames");
            }
        });

        Ok(stream)
    }

    pub async fn get_simple<I, O, E, EC>(
        &self,
        headers: Headers<M>,
        input_stream: Stream<I, EC>,
    ) -> Result<O, E>
    where
        I: Serialize + Send + 'static + Debug,
        O: DeserializeOwned + Send + 'static,
        EC: Debug + Display + std::error::Error + Send + Sync + 'static + Serialize,
        E: Debug + Display + std::error::Error + Send + Sync + 'static + DeserializeOwned,
    {
        let stream = self.get_inner(headers, input_stream).await?;
        let mut call_result_stream =
            FramedRead::new(stream, crate::DefaultCodec::<Result<O, E>>::default());
        match call_result_stream.next().await {
            Some(Ok(frame)) => frame,
            Some(Err(err)) => Err(Error::from(anyhow::anyhow!(
                "Could not deserialize message: {:?}",
                err
            ))),
            None => Err(Error::from(anyhow::anyhow!("Expected response, got EOF"))),
        }
    }

    pub async fn get_stream<I, O, E, EC, ES>(
        &self,
        headers: Headers<M>,
        input_stream: Stream<I, EC>,
    ) -> Result<Stream<O, ES>, E>
    where
        I: Serialize + Send + 'static + Debug,
        O: DeserializeOwned + Send + 'static,
        EC: Debug + Display + std::error::Error + Send + Sync + 'static + Serialize,
        ES: Debug + Display + std::error::Error + Send + Sync + 'static + DeserializeOwned,
        E: Debug + Display + std::error::Error + Send + Sync + 'static + DeserializeOwned,
    {
        let stream = self.get_inner(headers, input_stream).await?;

        let mut call_result_stream =
            FramedRead::new(stream, crate::DefaultCodec::<Result<(), E>>::default());
        match call_result_stream.next().await {
            Some(Ok(frame)) => match frame {
                Ok(()) => {
                    let server_stream = FramedRead::new(
                        call_result_stream.into_inner(),
                        crate::DefaultCodec::<Result<O, ES>>::default(),
                    );
                    let server_stream = server_stream.map_err(Error::from).and_then(ready);
                    Ok(Box::pin(server_stream))
                }
                Err(e) => Err(e),
            },
            Some(Err(err)) => Err(Error::from(anyhow::anyhow!(
                "Could not deserialize message: {:?}",
                err
            ))),
            None => Err(Error::from(anyhow::anyhow!("Expected response, got EOF"))),
        }
    }
}
