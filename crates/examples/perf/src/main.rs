use ar_pe_ce::{Result, Stream};
use futures::{FutureExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use anyhow::Context;

#[derive(Debug, Deserialize, Serialize)]
struct Row {
    payload: Vec<u8>,
}


#[ar_pe_ce::rpc]
trait Performance {
    #[rpc(server_streaming)]
    async fn get_data(&self, arg: ()) -> Result<Stream<Row>>;
}

struct HelloImpl;

#[ar_pe_ce::async_trait]
impl Performance for HelloImpl {
    #[tracing::instrument(skip(self))]
    async fn get_data(&self, _: ()) -> Result<Stream<Row>> {
        let stream = async_stream::stream! {
            loop {
                yield Ok(Row { payload: vec![0; 10_000_000] }); // 10 MB
            }
        };

        Ok(Box::pin(stream))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().pretty().compact().init();

    let mut client = tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let client = PerformanceClient::new("http://localhost:3000".parse()?);

        let mut data_stream = client.get_data(()).await.context("Could not get data stream")?;
        //Take only one chunk
        match data_stream.try_next().await.context("Could not retrieve message from data stream") {
            Err(e) => {
                tracing::error!("{:#?}", e);
            },
            Ok(Some(s)) => {
                tracing::info!(?s);
            },
            _ => ()
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        //Then we sleep
        // Lets see if our internal buffer wont be flooded with messages

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
