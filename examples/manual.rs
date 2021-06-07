// use ar_pe_ce::{Error, Result, Stream};
// use futures::{FutureExt, StreamExt, TryStreamExt};
// use serde::{Deserialize, Serialize};
// use tokio_stream::wrappers::IntervalStream;

// //Serializes and deserializes to MessagePack
// #[derive(Debug, Deserialize, Serialize)]
// pub struct Hello {
//     hello: String,
// }

// #[derive(Debug, Deserialize, Serialize)]
// pub struct World {
//     world: String,
// }

// #[ar_pe_ce::async_trait]
// pub trait HelloService {
//     async fn hello(&self, hello: Hello) -> Result<Stream<World>>;
//     async fn world(&self) -> Result<World>;
//     async fn world_many(&self, hello: Hello, world: World) -> Result<()>;
//     async fn foo(&self, hello: Stream<Hello>) -> Result<World>;
//     async fn bidi(&self, hello: Stream<Hello>) -> Result<Stream<World>>;
// }
// #[doc(hidden)]
// #[allow(non_snake_case)]
// pub mod HelloService_ar_pe_ce {
//     use super::*;
//     use ar_pe_ce::re::*;
//     use ar_pe_ce::*;
//     #[allow(non_camel_case_types)]
//     #[derive(ar_pe_ce :: re :: Deserialize, ar_pe_ce :: re :: Serialize, Debug)]
//     pub enum Methods {
//         hello {
//             hello: Hello,
//         },
//         world {},
//         world_many {
//             hello: Hello,
//             world: World,
//         },
//         foo {
//             hello: std::marker::PhantomData<Hello>,
//         },
//         bidi {
//             hello: std::marker::PhantomData<Hello>,
//         },
//     }
//     #[ar_pe_ce::re::instrument(skip(socket, handler))]
//     pub async fn handle(
//         socket: TcpStream,
//         handler: std::sync::Arc<impl HelloService>,
//     ) -> Result<()> {
//         let (stream, sink) = socket.into_split();
//         let mut header_stream = FramedRead::new(stream, MsgPack::<Headers<Methods>>::new());
//         let headers = header_stream
//             .try_next()
//             .await
//             .map_err(Error::from)?
//             .context("Expected headers")?;
//         tracing::info!(?headers);
//         match headers.method {
//             Methods::hello { hello, .. } => {
//                 let res = handler.hello(hello).await;
//                 let res = res.map_err(|e| Status::internal(e));
//                 match res {
//                     Ok(mut res) => {
//                         tokio::spawn(async move {
//                             let mut server_stream =
//                                 FramedWrite::new(sink, MsgPack::<Result<World, Status>>::new());
//                             let mut res = res.map_err(|e| Status::internal(e)).map(|e| Ok(e));
//                             server_stream.send_all(&mut res).await
//                         });
//                     }
//                     Err(err) => {
//                         let mut server_stream = FramedWrite::new(sink, MsgPack::<Status>::new());
//                         server_stream.send(err).await?;
//                     }
//                 }
//             }
//             Methods::world { .. } => {
//                 let res = handler.world().await;
//                 let res = res.map_err(|e| Status::internal(e));
//                 match res {
//                     Ok(mut res) => {
//                         let mut server_stream = FramedWrite::new(
//                             sink,
//                             MsgPack::<ar_pe_ce::Result<World, Status>>::new(),
//                         );
//                         server_stream.send(Ok(res)).await?;
//                     }
//                     Err(err) => {}
//                 }
//             }
//             Methods::world_many { hello, world, .. } => {
//                 let res = handler.world_many(hello, world).await;
//                 let res = res.map_err(|e| Status::internal(e));
//                 match res {
//                     Ok(mut res) => {
//                         let mut server_stream =
//                             FramedWrite::new(sink, MsgPack::<ar_pe_ce::Result<(), Status>>::new());
//                         server_stream.send(Ok(res)).await?;
//                     }
//                     Err(err) => {}
//                 }
//             }
//             Methods::foo { .. } => {
//                 let res = handler
//                     .foo({
//                         let client_stream =
//                             FramedRead::new(header_stream.into_inner(), MsgPack::<Hello>::new());
//                         Box::pin(client_stream)
//                     })
//                     .await;
//                 let res = res.map_err(|e| Status::internal(e));
//                 match res {
//                     Ok(mut res) => {
//                         let mut server_stream = FramedWrite::new(
//                             sink,
//                             MsgPack::<ar_pe_ce::Result<World, Status>>::new(),
//                         );
//                         server_stream.send(Ok(res)).await?;
//                     }
//                     Err(err) => {}
//                 }
//             }
//             Methods::bidi { .. } => {
//                 let res = handler
//                     .bidi({
//                         let client_stream =
//                             FramedRead::new(header_stream.into_inner(), MsgPack::<Hello>::new());
//                         Box::pin(client_stream)
//                     })
//                     .await;
//                 let res = res.map_err(|e| Status::internal(e));
//                 match res {
//                     Ok(mut res) => {
//                         tokio::spawn(async move {
//                             let mut server_stream =
//                                 FramedWrite::new(sink, MsgPack::<Result<World, Status>>::new());
//                             let mut res = res.map_err(|e| Status::internal(e)).map(|e| Ok(e));
//                             server_stream.send_all(&mut res).await
//                         });
//                     }
//                     Err(err) => {
//                         let mut server_stream = FramedWrite::new(sink, MsgPack::<Status>::new());
//                         server_stream.send(err).await?;
//                     }
//                 }
//             }
//         }
//         Ok(())
//     }
// }
// pub struct HelloServiceClient<A: tokio::net::ToSocketAddrs + Copy + Send + Sync>(
//     ar_pe_ce::ClientInner<A, HelloService_ar_pe_ce::Methods>,
// );
// impl<A: tokio::net::ToSocketAddrs + Copy + Send + Sync> HelloServiceClient<A> {
//     pub fn new(addr: A) -> Self {
//         Self(ar_pe_ce::ClientInner::new(addr))
//     }
// }
// #[ar_pe_ce::async_trait]
// impl<A: tokio::net::ToSocketAddrs + Copy + Send + Sync> HelloService for HelloServiceClient<A> {
//     async fn hello(&self, hello: Hello) -> Result<Stream<World>> {
//         use ar_pe_ce::re::*;
//         let mut res = self
//             .0
//             .get(
//                 ar_pe_ce::Headers {
//                     method: HelloService_ar_pe_ce::Methods::hello { hello },
//                     headers: Default::default(),
//                 },
//                 Box::pin(empty()) as ar_pe_ce::Stream<()>,
//             )
//             .await?;
//         Ok(res)
//     }
//     async fn world(&self) -> Result<World> {
//         use ar_pe_ce::re::*;
//         let mut res = self
//             .0
//             .get(
//                 ar_pe_ce::Headers {
//                     method: HelloService_ar_pe_ce::Methods::world {},
//                     headers: Default::default(),
//                 },
//                 Box::pin(empty()) as ar_pe_ce::Stream<()>,
//             )
//             .await?;
//         let res = res.try_next().await?.context("Expected response")?;
//         Ok(res)
//     }
//     async fn world_many(&self, hello: Hello, world: World) -> Result<()> {
//         use ar_pe_ce::re::*;
//         let mut res = self
//             .0
//             .get(
//                 ar_pe_ce::Headers {
//                     method: HelloService_ar_pe_ce::Methods::world_many { hello, world },
//                     headers: Default::default(),
//                 },
//                 Box::pin(empty()) as ar_pe_ce::Stream<()>,
//             )
//             .await?;
//         let res = res.try_next().await?.context("Expected response")?;
//         Ok(res)
//     }
//     async fn foo(&self, hello: Stream<Hello>) -> Result<World> {
//         use ar_pe_ce::re::*;
//         let mut res = self
//             .0
//             .get(
//                 ar_pe_ce::Headers {
//                     method: HelloService_ar_pe_ce::Methods::foo {
//                         hello: Default::default(),
//                     },
//                     headers: Default::default(),
//                 },
//                 hello,
//             )
//             .await?;
//         let res = res.try_next().await?.context("Expected response")?;
//         Ok(res)
//     }
//     async fn bidi(&self, hello: Stream<Hello>) -> Result<Stream<World>> {
//         use ar_pe_ce::re::*;
//         let mut res = self
//             .0
//             .get(
//                 ar_pe_ce::Headers {
//                     method: HelloService_ar_pe_ce::Methods::bidi {
//                         hello: Default::default(),
//                     },
//                     headers: Default::default(),
//                 },
//                 hello,
//             )
//             .await?;
//         Ok(res)
//     }
// }

// struct HelloImpl;

// #[ar_pe_ce::rpc]
// impl HelloService for HelloImpl {
//     #[tracing::instrument(skip(self))]
//     async fn hello(&self, hello: Hello) -> Result<Stream<World>> {
//         tracing::info!(?hello);
//         let stream =
//             IntervalStream::new(tokio::time::interval(tokio::time::Duration::from_secs(1)))
//                 .map(|_| {
//                     let s = World {
//                         world: "Hello".into(),
//                     };
//                     Ok::<_, Error>(s)
//                 })
//                 .take(5);

//         let stream_err = futures::stream::once(futures::future::ready(
//             Err(anyhow::anyhow!("!!!"))
//         ));

//         let stream = stream.chain(stream_err);

//         Ok(Box::pin(stream))
//     }

//     #[tracing::instrument(skip(self))]
//     async fn world(&self) -> Result<World> {
//         Ok(World {
//             world: "World".into(),
//         })
//     }

//     #[tracing::instrument(skip(self))]
//     async fn world_many(&self, _hello: Hello, _world: World) -> Result<()> {
//         Ok(())
//     }

//     #[tracing::instrument(skip(self, hello))]
//     async fn foo(&self, mut hello: Stream<Hello>) -> Result<World> {
//         while let Some(hello) = hello.try_next().await? {
//             tracing::info!(?hello);
//         }
//         Ok(World {
//             world: "Foo".into(),
//         })
//     }

//     #[tracing::instrument(skip(self, hello))]
//     async fn bidi(&self, hello: Stream<Hello>) -> Result<Stream<World>> {
//         Ok(Box::pin(hello.map_ok(|h| {
//             tracing::info!(?h);

//             World {
//                 world: "Foo".into(),
//             }
//         })))
//     }
// }

// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     tracing_subscriber::fmt().pretty().compact().init();

//     let mut client = tokio::spawn(async {
//         tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//         let client = HelloServiceClient::new("localhost:3000");

//         let mut hello = client
//             .hello(Hello {
//                 hello: "Foo".into(),
//             })
//             .await?;
//         while let Some(s) = hello.next().await {
//             tracing::info!(?s);
//         }

//         let world = client.world().await?;

//         tracing::info!(?world);

//         let world = client
//             .world_many(Hello { hello: "".into() }, World { world: "".into() })
//             .await?;

//         tracing::info!(?world);

//         let it = vec![
//             Ok(Hello {
//                 hello: "Foo".into(),
//             }),
//             Ok(Hello {
//                 hello: "Bar".into(),
//             }),
//         ]
//         .into_iter();
//         let foo_result = client.foo(Box::pin(futures::stream::iter(it))).await?;

//         tracing::info!(?foo_result);

//         let it = vec![
//             Ok(Hello {
//                 hello: "Foo".into(),
//             }),
//             Ok(Hello {
//                 hello: "Bar".into(),
//             }),
//         ]
//         .into_iter();
//         let mut bidi = client.bidi(Box::pin(futures::stream::iter(it))).await?;

//         while let Some(s) = bidi.try_next().await? {
//             tracing::info!(?s);
//         }

//         Ok::<_, anyhow::Error>(())
//     })
//     .fuse();

//     let server = HelloImpl.serve("0.0.0.0:3000").fuse();

//     futures::pin_mut!(server);

//     futures::select! {
//         server = server => server?,
//         client = client => client??
//     };

//     Ok(())
// }

fn main() {}
