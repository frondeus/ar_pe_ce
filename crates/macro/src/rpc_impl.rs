use crate::utils::{calculate_item_mod, crate_};
use quote::quote;
use syn::{ItemImpl, Path, Type};

pub struct RpcImpl {
    item: ItemImpl,
    name: Box<Type>,
    mod_path: Path,
}

pub fn analysis(item: ItemImpl) -> RpcImpl {
    let name = item.self_ty.clone();
    let trait_path = &item
        .trait_
        .as_ref()
        .expect("Expected impl Trait for Type")
        .1;
    let mod_path = calculate_item_mod(trait_path);
    RpcImpl {
        item,
        name,
        mod_path,
    }
}

impl RpcImpl {
    pub fn synthesis(self) -> proc_macro::TokenStream {
        let crate_ = crate_();
        let Self {
            item,
            name,
            mod_path,
        } = self;

        let result = quote! {
            #[#crate_::async_trait]
            #item

            impl #name {
                pub async fn serve_inner(self, server: #crate_::re::ServerInner) -> #crate_::re::anyhow::Result<()> {
                    use #crate_::re::*;
                    use #mod_path::Methods;
                    use std::sync::Arc;
                    let this = Arc::new(self);

                    loop {
                        let (socket, socket_addr) = server.listener.accept().await?;
                        tracing::debug!(?socket_addr, "New connection");
                        let this = this.clone();
                        tokio::spawn(async move {
                            Methods::handle(socket, this).await
                        });
                    }
                }
                pub async fn serve<A>(self, addr: A) -> #crate_::re::anyhow::Result<()>
                    where
                        A: tokio::net::ToSocketAddrs,
                {
                    use #crate_::re::*;
                    use #mod_path::Methods;
                    use std::sync::Arc;
                    let server = ServerInner::new(addr).await?;

                    self.serve_inner(server).await
                }
            }
        };

        result.into()
    }
}
