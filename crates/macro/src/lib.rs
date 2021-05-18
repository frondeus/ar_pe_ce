#![feature(drain_filter)]
extern crate proc_macro;

use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, parse_quote, FnArg, ItemTrait, PatType, TraitItem};

const SERVER_STREAMING: &str = "server_streaming";
const CLIENT_STREAMING: &str = "client_streaming";

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
                            .trim_start_matches('(')
                            .trim_end_matches(')')
                            .to_string()
                            .split(',')
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
            if method.sig.inputs.len() != 2 {
                panic!("All RPC methods have to take one argument");
            }
            let call = match method.sig.inputs.iter().nth(1) {
                Some(FnArg::Typed(PatType { .. })) if attrs.contains(CLIENT_STREAMING) => quote! {
                    self.#name(Box::pin(decode_stream(body))).await
                },
                Some(FnArg::Typed(PatType { ty, .. })) => {
                    quote! {{
                        let input: #ty = Box::pin(decode_stream(body))
                            .try_next()
                            .await
                            .context("Could not retrieve request body")?
                            .context("Expected argument")?;
                        self.#name(input).await
                    }}
                }
                _ => unreachable!(),
            };

            let method_handler = if attrs.contains(SERVER_STREAMING) {
                quote! {
                    Box::pin(encode_stream(#call.context("Could not handle the request")?))
                }
            } else {
                quote! {
                    Box::pin(encode_stream(once(ready(#call))))
                }
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
                Some(FnArg::Typed(PatType { pat, .. })) => quote! {
                    self.0.get(#method_name, once(ready(Ok(#pat))))
                },
                _ => unreachable!(),
            };

            let method_body = if attrs.contains(SERVER_STREAMING) {
                quote! {
                    Ok(#call.await.context("Could not send request to server")?)
                }
            } else {
                quote! {
                    Ok(#call.await.context("Could not send request to server")?
                        .try_next()
                        .await?
                        .context("Expected message")?)
                }
            };

            quote! {
                #sig {
                    use ar_pe_ce::re::*;
                    #method_body
                }
            }
        })
        .collect();

    item.items.push(parse_quote! {

        #[ar_pe_ce::re::instrument(skip(self))]
        async fn handle(
            self: std::sync::Arc<Self>,
            req: ar_pe_ce::re::Request<ar_pe_ce::re::Body>) ->
            ar_pe_ce::Result<ar_pe_ce::re::Response<ar_pe_ce::re::Body>> where Self: Send + Sync{

                use ar_pe_ce::{Stream, Error, re::*, encode_stream, decode_stream};

                let uri = req.uri().clone();
                let uri = uri.path();
                let body = req.into_body();

                tracing::info!(?uri, "Handling");

                let stream: Stream<Bytes> = match uri {
                    #(#method_variants,)*
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
            use ar_pe_ce::{Error, re::*};
            use std::sync::Arc;

            let service = Arc::new(self);

            let make_svc = make_service_fn(|_conn| {
                let service = service.clone();
                async move { Ok::<_, Error>(service_fn(move |req| service.clone().handle(req))) }
            });

            Server::bind(&addr).http2_only(true).serve(make_svc).await
                .context("Could not serve the RPC service")?;

            Ok(())
        }
    });

    let client_name = format_ident!("{}Client", item_name);

    let result = quote! {
        #[ar_pe_ce::async_trait]
        #item

        pub struct #client_name(ar_pe_ce::ClientInner);
        impl #client_name {
            pub fn new(url: ar_pe_ce::re::Url) -> Self {
                Self(ar_pe_ce::ClientInner::new(url))
            }
        }

        #[ar_pe_ce::async_trait]
        impl #item_name for #client_name {
            async fn serve(self, _addr: std::net::SocketAddr) -> ar_pe_ce::Result<()> where Self: Sized + Send + Sync + 'static {
                unimplemented!(concat!("Please implement ", stringify!(#item_name), " manually. This is a client implementation"))
            }

            async fn handle(self: std::sync::Arc<Self>, req: ar_pe_ce::re::Request<ar_pe_ce::re::Body>) ->
                ar_pe_ce::Result<ar_pe_ce::re::Response<ar_pe_ce::re::Body>> where Self: Send + Sync {
                unimplemented!(concat!("Please implement ", stringify!(#item_name), " manually. This is a client implementation"))
            }

            #(#client_methods)*
        }
    };

    result.into()
}
