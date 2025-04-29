#![doc = include_str!("../README.md")]

mod args;
mod decl;
mod imp;
mod ty;

use proc_macro::TokenStream;
use syn::{Error, parse_macro_input};

#[proc_macro_attribute]
pub fn extern_trait(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        let args = parse_macro_input!(args as args::Proxy);
        decl::expand(args.into(), parse_macro_input!(input))
    } else {
        imp::expand(parse_macro_input!(input))
    }
    .unwrap_or_else(Error::into_compile_error)
    .into()
}
