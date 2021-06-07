extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, Item};

#[proc_macro_attribute]
pub fn rpc(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    match item {
        Item::Trait(item) => rpc_trait::analysis(item).synthesis(),
        Item::Impl(item) => rpc_impl::analysis(item).synthesis(),
        _ => panic!("ar_pe_ce::rpc macro can be used only for trait or impl"),
    }
}

mod rpc_impl;
mod rpc_trait;
mod utils;
