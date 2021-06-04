#![allow(clippy::blacklisted_name)]

use ar_pe_ce::{Result, Stream};
use core::sync::atomic::AtomicBool;
pub use core::sync::atomic::Ordering;
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};

pub static MANY_ARGUMENTS: AtomicBool = AtomicBool::new(false);
pub static NO_ARGUMENTS: AtomicBool = AtomicBool::new(false);
pub static ONE_ARGUMENT: AtomicBool = AtomicBool::new(false);

//Serializes and deserializes to MessagePack
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Foo {
    pub foo: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Bar {
    pub bar: String,
}

#[ar_pe_ce::rpc]
pub trait Service {
    async fn many_arguments(&self, foo: Foo, bar: Bar) -> Result<()>;
    async fn no_arguments(&self) -> Result<()>;
    async fn one_argument(&self, foo: Foo) -> Result<()>;

    async fn return_one(&self) -> Result<Foo>;
    async fn return_error(&self) -> Result<Foo>;

    #[rpc(client_streaming)] // And for client streaming, last parameter is a stream
    async fn client_streaming(&self, foo: Stream<Foo>) -> Result<Bar>;

    #[rpc(client_streaming)]
    async fn one_arg_client_streaming(&self, bar: Bar, foo: Stream<Foo>) -> Result<Bar>;

    #[rpc(server_streaming)]
    async fn server_streaming(&self) -> Result<Stream<Bar>>;

    #[rpc(server_streaming)]
    async fn server_streaming_return_error(&self) -> Result<Stream<Bar>>;

    #[rpc(server_streaming)]
    async fn server_streaming_error(&self) -> Result<Stream<Bar>>;
}

pub struct Server;

#[ar_pe_ce::rpc]
impl Service for Server {
    #[tracing::instrument(skip(self))]
    async fn many_arguments(&self, _foo: Foo, _bar: Bar) -> Result<()> {
        MANY_ARGUMENTS.store(true, Ordering::SeqCst);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    async fn no_arguments(&self) -> Result<()> {
        NO_ARGUMENTS.store(true, Ordering::SeqCst);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    async fn one_argument(&self, _foo: Foo) -> Result<()> {
        ONE_ARGUMENT.store(true, Ordering::SeqCst);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    async fn return_one(&self) -> Result<Foo> {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        Ok(Foo {
            foo: "return one".into(),
        })
    }
    #[tracing::instrument(skip(self))]
    async fn return_error(&self) -> Result<Foo> {
        Err(ar_pe_ce::anyhow!("return error"))
    }
    #[tracing::instrument(skip(self, foo))]
    async fn client_streaming(&self, foo: Stream<Foo>) -> Result<Bar> {
        let bar = foo
            .try_fold(String::new(), |acc, it| async move {
                Ok(format!("{}{}", acc, it.foo))
            })
            .await?;
        Ok(Bar { bar })
    }
    #[tracing::instrument(skip(self, foo))]
    async fn one_arg_client_streaming(&self, bar: Bar, mut foo: Stream<Foo>) -> Result<Bar> {
        while let Some(foo) = foo.try_next().await? {
            tracing::info!(?foo);
        }
        Ok(bar)
    }
    #[tracing::instrument(skip(self))]
    async fn server_streaming(&self) -> Result<Stream<Bar>> {
        let it = vec![
            Ok(Bar { bar: "a".into() }),
            Ok(Bar { bar: "b".into() }),
            Ok(Bar { bar: "c".into() }),
        ];
        Ok(Box::pin(futures::stream::iter(it.into_iter())))
    }
    #[tracing::instrument(skip(self))]
    async fn server_streaming_return_error(&self) -> Result<Stream<Bar>> {
        Err(ar_pe_ce::anyhow!("return error"))
    }
    #[tracing::instrument(skip(self))]
    async fn server_streaming_error(&self) -> Result<Stream<Bar>> {
        let it = vec![
            Ok(Bar { bar: "a".into() }),
            Ok(Bar { bar: "b".into() }),
            Err(ar_pe_ce::anyhow!("server stream error")),
        ];
        Ok(Box::pin(futures::stream::iter(it.into_iter())))
    }
}
