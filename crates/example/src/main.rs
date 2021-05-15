use ar_pe_ce::{Error, Result, Stream};
use futures::{FutureExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::IntervalStream;

#[derive(Debug, Deserialize, Serialize)]
struct Hello {
    hello: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct World {
    world: String,
}

#[ar_pe_ce::rpc]
trait HelloService {
    #[rpc(server_streaming)]
    async fn hello(&self, hello: Hello) -> Result<Stream<World>>;

    async fn world(&self) -> Result<World>;

    #[rpc(client_streaming)]
    async fn foo(&self, hello: Stream<Hello>) -> Result<World>;

    #[rpc(client_streaming, server_streaming)]
    async fn bidi(&self, hello: Stream<Hello>) -> Result<Stream<World>>;
}

struct HelloImpl;

#[async_trait::async_trait]
impl HelloService for HelloImpl {
    #[tracing::instrument(skip(self))]
    async fn hello(&self, hello: Hello) -> Result<Stream<World>> {
        tracing::info!(?hello);
        let stream =
            IntervalStream::new(tokio::time::interval(tokio::time::Duration::from_secs(1)))
                .map(|_| {
                    let s = World {
                        world: "Hello".into(),
                    };
                    Ok::<_, Error>(s)
                })
                .take(5);

        Ok(Box::pin(stream))
    }

    async fn world(&self) -> Result<World> {
        Ok(World {
            world: "World".into(),
        })
    }

    async fn foo(&self, mut hello: Stream<Hello>) -> Result<World> {
        while let Some(hello) = hello.try_next().await? {
            tracing::info!(?hello);
        }
        Ok(World {
            world: "Foo".into(),
        })
    }

    async fn bidi(&self, hello: Stream<Hello>) -> Result<Stream<World>> {
        Ok(Box::pin(hello.map_ok(|h| {
            tracing::info!(?h);

            World {
                world: "Foo".into()
            }
        })))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().pretty().compact().init();

    let mut client = tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let client = HelloServiceClient::new("http://localhost:3000".parse()?);

        let mut hello = client
            .hello(Hello {
                hello: "Foo".into(),
            })
            .await?;
        while let Some(s) = hello.try_next().await? {
            tracing::info!(?s);
        }

        let world = client.world().await?;

        tracing::info!(?world);

        let it = vec![
            Ok(Hello {
                hello: "Foo".into(),
            }),
            Ok(Hello {
                hello: "Bar".into(),
            }),
        ]
        .into_iter();
        let foo = client.foo(Box::pin(futures::stream::iter(it))).await?;

        tracing::info!(?foo);

        let it = vec![
            Ok(Hello {
                hello: "Foo".into(),
            }),
            Ok(Hello {
                hello: "Bar".into(),
            }),
        ]
        .into_iter();
        let mut bidi = client.bidi(Box::pin(futures::stream::iter(it))).await?;

        while let Some(s) = bidi.try_next().await? {
            tracing::info!(?s);
        }

        Ok::<_, anyhow::Error>(())
    })
    .fuse();

    use std::net::SocketAddr;

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    let server = HelloImpl.serve(addr).fuse();

    futures::pin_mut!(server);

    futures::select! {
        server = server => server?,
        client = client => client??
    };

    Ok(())
}
