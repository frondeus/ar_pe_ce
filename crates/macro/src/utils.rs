use itertools::Itertools;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::format_ident;
use quote::quote;
use syn::{GenericArgument, Ident, Path, PathArguments, PathSegment};

pub fn crate_() -> proc_macro2::TokenStream {
    let found_crate = crate_name("ar_pe_ce").expect("ar_pe_ce is not present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => quote! { ::ar_pe_ce },
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote! { ::#ident }
        }
    }
}

pub fn calculate_item_mod(path: &Path) -> Path {
    let mut n = path.clone();

    let last = n.segments.last_mut().expect("Last segment");

    last.ident = format_ident!("{}_ar_pe_ce", last.ident);

    n
}

pub fn extract_from_path(ty: &syn::Type) -> Option<&Path> {
    match *ty {
        syn::Type::Path(ref tp) if tp.qself.is_none() => Some(&tp.path),
        _ => None,
    }
}

const STREAM: &[&str] = &["Stream", "ar_pe_ce::Stream"];
const RESULT: &[&str] = &["Result", "ar_pe_ce::Result"];

pub fn angle_brackets(segment: &PathSegment) -> Option<impl Iterator<Item = &syn::Type>> {
    let type_params = &segment.arguments;
    let g = match *type_params {
        PathArguments::AngleBracketed(ref params) => params.args.iter(),
        _ => return None,
    };
    Some(g.filter_map(|g| match *g {
        GenericArgument::Type(ref ty) => Some(ty),
        _ => None,
    }))
}

pub fn extract_segment<'a>(
    path: &'a Path,
    pat: &'static [&'static str],
) -> Option<&'a PathSegment> {
    let idents_of_path = path
        .segments
        .iter()
        .into_iter()
        .map(|p| p.ident.to_string())
        .join("::");

    pat.iter().find(|s| idents_of_path.contains(*s))?;
    path.segments.last()
}

pub fn extract_type_from_result(ty: &syn::Type) -> Option<(&syn::Type, Option<&syn::Type>)> {
    let path = extract_from_path(ty)?;
    let pair_path_segment = extract_segment(path, RESULT)?;
    let mut it = angle_brackets(pair_path_segment)?;

    let t = it.next()?;
    let e = it.next();
    Some((t, e))
}

pub fn extract_type_from_stream(ty: &syn::Type) -> Option<(&syn::Type, Option<&syn::Type>)> {
    let path = extract_from_path(ty)?;
    let pair_path_segment = extract_segment(path, STREAM)?;
    let mut it = angle_brackets(pair_path_segment)?;

    let t = it.next()?;
    let e = it.next();
    Some((t, e))
}

pub fn debug_arg(result: &proc_macro2::TokenStream) {
    if std::env::var("AR_PE_CE_DEBUG").is_ok() {
        let path = "/tmp/ar_pe_ce_debug.rs";

        std::fs::File::create(path)
            .and_then(|mut f| {
                use std::io::Write;
                write!(&mut f, "{}", result)
            })
            .expect("Could not write debug file");
        let _output = std::process::Command::new("rustfmt")
            .arg("--edition")
            .arg("2018")
            .arg(path)
            .output()
            .expect("Rustfmt failed");
    }
}
