use anyhow::Context;
use anyhow::Result;

mod service;
use service::*;
mod tracing_utils;
use tracing_utils::*;

async fn serve() -> Result<ServiceClient<String>> {
    let server = ar_pe_ce::ServerInner::new("0.0.0.0:0").await?;
    let port = server.port()?;
    tokio::spawn(async move { Server.serve_inner(server).await });
    let addr = format!("localhost:{}", port);
    let client = ServiceClient::new(addr);
    Ok(client)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_one_argument() -> Result<()> {
    init_tracing();
    ONE_ARGUMENT.store(false, Ordering::SeqCst);
    let client = serve().await?;
    client.one_argument(Foo { foo: "foo".into() }).await?;
    assert_eq!(ONE_ARGUMENT.load(Ordering::SeqCst), true);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_no_arguments() -> Result<()> {
    init_tracing();
    NO_ARGUMENTS.store(false, Ordering::SeqCst);
    let client = serve().await?;
    client.no_arguments().await?;
    assert_eq!(NO_ARGUMENTS.load(Ordering::SeqCst), true);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_many_arguments() -> Result<()> {
    init_tracing();
    MANY_ARGUMENTS.store(false, Ordering::SeqCst);
    let client = serve().await?;
    client
        .many_arguments(Foo { foo: "foo".into() }, Bar { bar: "bar".into() })
        .await?;
    assert_eq!(MANY_ARGUMENTS.load(Ordering::SeqCst), true);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_return_one() -> Result<()> {
    init_tracing();
    let client = serve().await?;
    let ret = client.return_one().await?;
    assert_eq!(
        ret,
        Foo {
            foo: "return one".into()
        }
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_return_error() -> Result<()> {
    init_tracing();
    let client = serve().await?;
    let ret = client.return_error().await.err().context("Expects error")?;
    assert_eq!(
        format!("{:?}", anyhow::Error::from(ret)),
        "Internal server error\n\nCaused by:\n    return error"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_client_streaming() -> Result<()> {
    init_tracing();
    let client = serve().await?;
    let it = vec![
        Ok(Foo { foo: "1".into() }),
        Ok(Foo { foo: "2".into() }),
        Ok(Foo { foo: "3".into() }),
    ];
    let ret = client
        .client_streaming(Box::pin(futures::stream::iter(it.into_iter())))
        .await?;
    assert_eq!(ret, Bar { bar: "123".into() });
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_client_streaming_err() -> Result<()> {
    init_tracing();
    let client = serve().await?;
    let it = vec![
        Ok(Foo { foo: "1".into() }),
        Ok(Foo { foo: "2".into() }),
        Ok(Foo { foo: "3".into() }),
        Err(ar_pe_ce::anyhow!("client stream error")),
    ];
    let ret = client
        .client_streaming(Box::pin(futures::stream::iter(it.into_iter())))
        .await
        .err()
        .context("Expects error")?;
    assert_eq!(
        format!("{:?}", anyhow::Error::from(ret)),
        "Internal server error\n\nCaused by:\n    client stream error"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_one_arg_cs() -> Result<()> {
    init_tracing();
    let client = serve().await?;
    let it = vec![
        Ok(Foo { foo: "1".into() }),
        Ok(Foo { foo: "2".into() }),
        Ok(Foo { foo: "3".into() }),
    ];
    let ret = client
        .one_arg_client_streaming(
            Bar { bar: "bar".into() },
            Box::pin(futures::stream::iter(it.into_iter())),
        )
        .await?;
    assert_eq!(ret, Bar { bar: "bar".into() });
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_ok_server_streaming() -> Result<()> {
    use futures::TryStreamExt;
    init_tracing();
    let client = serve().await?;
    let ret: Vec<_> = client.server_streaming().await?.try_collect().await?;
    assert_eq!(
        ret,
        vec![
            Bar { bar: "a".into() },
            Bar { bar: "b".into() },
            Bar { bar: "c".into() }
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_streaming_return_error() -> Result<()> {
    init_tracing();
    let client = serve().await?;
    let ret = client
        .server_streaming_return_error()
        .await
        .err()
        .context("Expects error")?;
    assert_eq!(
        format!("{:?}", anyhow::Error::from(ret)),
        "Internal server error\n\nCaused by:\n    return error"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_streaming_error() -> Result<()> {
    use futures::StreamExt;
    init_tracing();
    let client = serve().await?;
    let ret: Vec<_> = client.server_streaming_error().await?.collect().await;
    assert_eq!(ret.len(), 3);
    let mut ret = ret.into_iter();
    let first = ret.next().unwrap().expect("First elem");
    let second = ret.next().unwrap().expect("Second elem");
    assert_eq!(first, Bar { bar: "a".into() });
    assert_eq!(second, Bar { bar: "b".into() });
    let third = ret.next().unwrap().expect_err("Third elem");
    assert_eq!(
        format!("{:?}", anyhow::Error::from(third)),
        "Internal server error\n\nCaused by:\n    server stream error"
    );
    Ok(())
}
