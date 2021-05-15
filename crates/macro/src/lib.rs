#![feature(drain_filter)]
extern crate proc_macro;

use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, parse_quote, FnArg, ItemTrait, PatType, TraitItem};

const SERVER_STREAMING: &'static str = "server_streaming";
const CLIENT_STREAMING: &'static str = "client_streaming";

#[proc_macro_attribute]
pub fn rpc(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(item as ItemTrait);
    let item_name = item.ident.clone();

    let methods: Vec<_> = item
        .items
        .iter_mut()
        .filter_map(|item| match item {
            TraitItem::Method(method) => {
                let rpc_attrs: HashSet<_> = method
                    .attrs
                    .drain_filter(|attr| attr.path == parse_quote! { rpc })
                    .flat_map(|attr| {
                        attr.tokens
                            .to_string()
                            .trim_start_matches("(")
                            .trim_end_matches(")")
                            .to_string()
                            .split(",")
                            .map(|s| s.trim().to_string())
                            .collect::<Vec<String>>()
                    })
                    .collect();
                Some((method, rpc_attrs))
            }
            _ => None,
        })
        .collect();

    let method_variants: Vec<_> = methods
        .iter()
        .map(|(method, attrs)| {
            let name = method.sig.ident.clone();
            let method_uri = format!("/{}", name);
            if method.sig.inputs.len() > 2 {
                panic!("All RPC methods have to take maximum one argument");
            }
            let call = match method.sig.inputs.iter().nth(1) {
                Some(FnArg::Typed(PatType { pat, .. })) if attrs.contains(CLIENT_STREAMING) => {
                    quote! {{
                        let #pat = Box::pin(body);
                        self.#name(#pat).await
                    }
                    }
                }
                Some(FnArg::Typed(PatType { pat, ty, .. })) => {
                    quote! {{
                        let #pat: #ty = body.try_next().await?.context("Expected argument")?;
                        self.#name(#pat).await
                    }
                    }
                }
                _ => quote! {
                    self.#name().await
                },
            };

            let method_handler = if attrs.contains(SERVER_STREAMING) {
                quote! {{
                    let stream = #call?
                    .and_then(|res| async move {
                        rmp_serde::to_vec(&res).map_err(Error::from)
                    });
                    Box::pin(stream)
                }}
            } else {
                quote! {{
                    let res = #call
                        .and_then(|res| rmp_serde::to_vec(&res).map_err(Error::from));
                    Box::pin(futures::stream::once(async move { res }))
                }}
            };
            quote! {
                #method_uri => #method_handler
            }
        })
        .collect();

    let client_methods: Vec<_> = methods
        .iter()
        .map(|(method, attrs)| {
            let method_name = method.sig.ident.clone().to_string();
            let sig = method.sig.clone();

            let call = match method.sig.inputs.iter().nth(1) {
                Some(FnArg::Typed(PatType { pat, .. })) if attrs.contains(CLIENT_STREAMING) => {
                    quote! {
                        self.0.get(#method_name, #pat)
                    }
                }
                Some(FnArg::Typed(PatType { pat, .. })) => {
                    quote! {{
                        let arg_stream = futures::stream::once(futures::future::ready(Ok(#pat)));
                        self.0.get(#method_name, arg_stream)
                    }}
                }
                _ => quote! {
                    self.0.get_empty(#method_name)
                },
            };

            let method_body = if attrs.contains(SERVER_STREAMING) {
                quote! {
                    Ok(Box::pin(#call.await?))
                }
            } else {
                quote! {
                    use ar_pe_ce::{futures::TryStreamExt, anyhow::Context};

                    let item = #call.await?
                        .try_next()
                        .await?
                        .context("Expected message")?;
                    Ok(item)
                }
            };

            quote! {
                #sig {
                    #method_body
                }
            }
        })
        .collect();

    item.items.push(parse_quote! {

        #[ar_pe_ce::tracing::instrument(skip(self))]
        async fn handle(
            self: std::sync::Arc<Self>,
            req: ar_pe_ce::hyper::Request<ar_pe_ce::hyper::Body>) ->
            ar_pe_ce::Result<ar_pe_ce::hyper::Response<ar_pe_ce::hyper::Body>> where Self: Send + Sync {

                use ar_pe_ce::{Stream, hyper::{Response, Body, StatusCode}, futures::{self, future}, Error, anyhow::Context};

                let uri = req.uri().clone();
                let uri = uri.path();
                let mut body = req.into_body()
                    .try_filter(|i| future::ready(!i.is_empty()))
                    .map_err(Error::from)
                    .and_then(|it| future::ready(rmp_serde::from_read_ref(&it).map_err(Error::from)));

                ar_pe_ce::tracing::info!(?uri, "Handling");

                let stream: Stream<Vec<u8>> = match uri {
                    #(#method_variants),*
                    _ => {
                        let mut res = Response::new(Body::empty());
                        *res.status_mut() = StatusCode::NOT_FOUND;
                        return Ok(res);
                    }
                };

                Ok(Response::new(Body::wrap_stream(stream)))
            }
    });

    item.items.push(parse_quote! {
        async fn serve(self, addr: std::net::SocketAddr) -> ar_pe_ce::Result<()> where Self: Sized + Send + Sync + 'static {
            use ar_pe_ce::{hyper::{service::{service_fn, make_service_fn}, Server}};
            let service = std::sync::Arc::new(self);

            let make_svc = make_service_fn(|_conn| {
                let service = service.clone();
                async move { Ok::<_, ar_pe_ce::anyhow::Error>(service_fn(move |req| service.clone().handle(req))) }
            });

            Server::bind(&addr).http2_only(true).serve(make_svc).await?;

            Ok(())
        }
    });

    let client_name = format_ident!("{}Client", item_name);

    let result = quote! {
        #[ar_pe_ce::async_trait]
        #item

        pub struct #client_name(ar_pe_ce::ClientInner);
        impl #client_name {
            pub fn new(url: ar_pe_ce::url::Url) -> Self {
                Self(ar_pe_ce::ClientInner::new(url))
            }
        }

        #[ar_pe_ce::async_trait]
        impl #item_name for #client_name {
            async fn serve(self, _addr: std::net::SocketAddr) -> ar_pe_ce::Result<()> where Self: Sized + Send + Sync + 'static {
                unimplemented!(concat!("Please implement ", stringify!(#item_name), " manually. This is a client implementation"))
            }

            #(#client_methods)*
        }
    };

    result.into()
}
