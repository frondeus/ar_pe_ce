#![allow(clippy::large_enum_variant)]
use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, FnArg, Ident, ItemTrait, Pat, PatType, Path, Signature, TraitItem, Type};

use crate::utils::{crate_, debug_arg, extract_type_from_result, extract_type_from_stream};

pub struct RpcTrait {
    crate_: TokenStream,
    item: ItemTrait,
    name: Ident,
    mod_name: Ident,
    client_name: Ident,
    methods: Vec<RpcMethod>,
}

struct ClientStream {
    pat: Box<Pat>,
    output: Type,
    stream_error: Option<Type>,
}

enum OutputType {
    Simple {
        output: Type,
        error: Option<Type>,
    },
    Streaming {
        output: Type,
        stream_error: Option<Type>,
        error: Option<Type>,
    },
}

impl OutputType {
    fn return_(&self) -> TokenStream {
        match &self {
            OutputType::Simple { output, .. } => quote! {
                #output
            },
            OutputType::Streaming { .. } => quote! { () },
        }
    }

    fn error(&self) -> TokenStream {
        let error = match &self {
            OutputType::Simple { error, .. } => error,
            OutputType::Streaming { error, .. } => error,
        };

        error
            .as_ref()
            .map(|se| quote! { #se })
            .unwrap_or_else(|| quote! { Infallible })
    }
}

struct RpcMethod {
    name: Ident,
    sig: Signature,
    args: Vec<PatType>,
    stream: Option<ClientStream>,
    output_type: OutputType,
}

#[allow(non_snake_case)]
pub fn analysis(mut item: ItemTrait) -> RpcTrait {
    let name = item.ident.clone();

    let RPC_ATTR: Path = parse_quote! { rpc };

    let methods = item
        .items
        .iter_mut()
        .filter_map(|item| match item {
            TraitItem::Method(method) => {
                let rpc_attrs: HashSet<_> = method
                    .attrs
                    .iter()
                    .filter(|attr| attr.path == RPC_ATTR)
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

                method.attrs.retain(|attr| attr.path != RPC_ATTR);

                Some((method, rpc_attrs))
            }
            _ => None,
        })
        .map(|(method, attrs)| {
            let name = method.sig.ident.clone();
            let sig = method.sig.clone();
            let mut args: Vec<_> = method
                .sig
                .inputs
                .iter()
                .filter_map(|i| match i {
                    FnArg::Receiver(_) => None,
                    FnArg::Typed(t) => Some(t),
                })
                .cloned()
                .collect();

            let output_type = match &method.sig.output {
                syn::ReturnType::Default => {
                    panic!("return type must be `ar_pe_ce::Result<T>`")

                }
                syn::ReturnType::Type(_, ty) => {
                    let (ty, error) = extract_type_from_result(&ty).expect("return type must be `ar_pe_ce::Result<T>`");

                    if attrs.contains("server_streaming") {
                        let (ty, stream_error) = extract_type_from_stream(&ty)
                            .expect("with attribute `server_streaming` method must be `ar_pe_ce::Result<ar_pe_ce::Stream<T>>`");
                        OutputType::Streaming {
                            output: ty.clone(),
                            error: error.cloned(),
                            stream_error: stream_error.cloned()
                        }
                    } else {
                        OutputType::Simple {
                            error: error.cloned(),
                            output: ty.clone()
                        }
                    }
                }
            };

            let stream = attrs.contains("client_streaming").then(|| {
                let arg = args.pop().expect("with attribute `client_streaming` method must take at least one argument other than `&self`");
                let pat = &arg.pat;
                let (ty, stream_error) = extract_type_from_stream(&arg.ty)
                            .expect("with attribute `client_streaming` method's last argument must be `ar_pe_ce::Stream<T>`");
                ClientStream {
                    pat: pat.clone(),
                    output: ty.clone(),
                    stream_error: stream_error.cloned()
                }
            });

            RpcMethod {
                name,
                sig,
                args,
                stream,
                output_type
            }
        })
        .collect();

    let crate_ = crate_();

    RpcTrait {
        item,
        mod_name: format_ident!("{}_ar_pe_ce", name),
        client_name: format_ident!("{}Client", name),
        name,
        methods,
        crate_,
    }
}

impl RpcTrait {
    pub fn method_enum(&self) -> TokenStream {
        let crate_ = &self.crate_;
        let method_defs = self.methods.iter().map(|method| {
            let name = &method.name;
            let args = &method.args;
            quote! {
                #name {
                    #(#args),*
                }
            }
        });
        quote! {
            #[allow(non_camel_case_types)]
            #[derive(#crate_::re::Deserialize, #crate_::re::Serialize, Debug)]
            pub enum Methods {
                #(#method_defs),*
            }
        }
    }

    pub fn handler(&self) -> TokenStream {
        let crate_ = &self.crate_;
        let name = &self.name;

        let method_handlers =
            self.methods
            .iter()
            .map(|method| {
                let name = &method.name;
                let args: Vec<_> = method.args.iter().map(|a| &a.pat).collect();
                let client_stream = method
                    .stream
                    .as_ref()
                    .map(|ClientStream { output, stream_error, .. }| {
                        let comma = (!args.is_empty()).then(|| quote! {,}).unwrap_or_default();
                        quote! {
                            #comma
                            {
                                let client_stream = FramedRead::new(header_stream.into_inner(),
                                                                     DefaultCodec::<Result<#output, #stream_error> >::default());
                                let client_stream = client_stream.map_err(Error::from).and_then(ready);
                                Box::pin(client_stream)
                            }
                        }
                    })
                    .unwrap_or_default();

                let send = match &method.output_type {
                    OutputType::Simple { .. } => {
                        quote! {
                            return_response_stream.send(Ok(result)).await?;
                        }
                    }
                    OutputType::Streaming { output, stream_error, .. } => {
                        let stream_error = stream_error
                            .as_ref()
                            .map(|se| quote! { #se })
                            .unwrap_or_else(|| quote! { Infallible });
                        quote! {
                            return_response_stream
                                .send(Ok(())).await?;

                            let mut server_stream = FramedWrite::new(
                                return_response_stream.into_inner(),
                                DefaultCodec::<Result<#output, #stream_error>>::default()
                            );

                            let mut stream = result.map(Ok);
                            tokio::spawn(async move {
                                server_stream.send_all(&mut stream).await
                            });
                        }
                    }
                };

                let return_ = method.output_type.return_();
                let error = method.output_type.error();

                let error_handling = quote! {
                    let mut return_response_stream = FramedWrite::new(
                        sink,
                        DefaultCodec::<Result<#return_, #error>>::default()
                    );

                    match result {
                        Ok(result) => {
                            #send
                        },
                        Err(err) => {
                            return_response_stream
                                .send(Err(err)).await?;
                        }
                    }
                };

                quote! {
                    Self::#name { #(#args),* } => {
                        let result = handler.#name(#(#args),* #client_stream).await;

                        #error_handling
                    }
                }
            });

        quote! {
            impl Methods {
                #[#crate_::re::instrument(skip(socket, handler))]
                pub async fn handle(socket: TcpStream, handler: std::sync::Arc<impl #name>) -> Result<()> {
                    let (stream, sink) = socket.into_split();
                    let mut header_stream = FramedRead::new(stream, DefaultCodec::<Headers<Self>>::default());
                    let headers = header_stream.try_next().await.map_err(Error::from)?.context("Expected headers")?;


                    tracing::info!(?headers);

                     match headers.method {
                         #(#method_handlers),*
                     }

                    Ok(())
                }
            }
        }
    }

    pub fn client(&self) -> TokenStream {
        let crate_ = &self.crate_;
        let client_name = &self.client_name;
        let mod_name = &self.mod_name;
        let name = &self.name;

        let client_methods = self.methods.iter().map(|method| {
            let name = &method.name;
            let sig = &method.sig;
            let args = method.args.iter().map(|arg| &arg.pat);

            let streaming_arg = method
                .stream
                .as_ref()
                .map(|stream| {
                    let pat = &stream.pat;
                    quote! {
                        #pat
                    }
                })
                .unwrap_or_else(|| {
                    quote! {
                        Box::pin(empty()) as Stream<()>
                    }
                });

            let call = match &method.output_type {
                OutputType::Simple { .. } => {
                    format_ident!("get_simple")
                }
                OutputType::Streaming { .. } => {
                    format_ident!("get_stream")
                }
            };

            quote! {
                #sig {
                    use #crate_::re::*;

                    let mut res = self.0.#call(Headers {
                        method: #mod_name::Methods::#name {
                            #(#args),*
                        },
                        headers: Default::default()
                    }, #streaming_arg).await;

                    res
                }
            }
        });

        quote! {
            pub struct #client_name<A: tokio::net::ToSocketAddrs + Send + Sync>(
                ar_pe_ce::ClientInner<A, #mod_name::Methods>
            );
            impl<A: tokio::net::ToSocketAddrs + Send + Sync> #client_name<A> {
                pub fn new(addr: A) -> Self {
                    Self(ar_pe_ce::ClientInner::new(addr))
                }
            }

            #[#crate_::async_trait]
            impl<A: tokio::net::ToSocketAddrs + Send + Sync> #name for #client_name<A> {
                #(#client_methods)*
            }
        }
    }

    pub fn synthesis(self) -> proc_macro::TokenStream {
        let method_enum = self.method_enum();
        let handler = self.handler();
        let client = self.client();
        let item = self.item;
        let mod_name = self.mod_name;
        let crate_ = self.crate_;

        let result = quote! {
            #[#crate_::async_trait]
            #item

            #[doc(hidden)]
            #[allow(non_snake_case)]
            pub mod #mod_name {
                use super::*;
                use #crate_::re::*;

                #method_enum

                #handler
            }

            #client
        };

        debug_arg(&result);

        result.into()
    }
}
