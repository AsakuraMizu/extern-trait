use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, Ident, ItemStruct, ItemTrait, Result, ReturnType, Signature, TraitItem, Type,
    TypeParamBound, parse_quote,
};

use crate::ty::{SelfKind, TypeExt};

pub fn expand(proxy: ItemStruct, input: ItemTrait) -> Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            input.generics,
            "#[extern_trait] may not have generics",
        ));
    }

    let trait_name = &input.ident;
    let Some(unsafety) = &input.unsafety else {
        return Err(Error::new(
            Span::call_site(),
            "#[extern_trait] must be unsafe",
        ));
    };

    let proxy_name = &proxy.ident;
    let mut impl_content = TokenStream::new();

    let macro_name = format_ident!("__extern_trait_{}", trait_name);
    let mut macro_content = TokenStream::new();

    let symbol_prefix = format!(
        "__extern_trait_{}_{}_{}_{}",
        std::env::var("CARGO_PKG_NAME")
            .as_deref()
            .unwrap_or("<unknown>"),
        std::env::var("CARGO_PKG_VERSION")
            .as_deref()
            .unwrap_or("<unknown>"),
        std::env::var("CARGO_CRATE_NAME")
            .as_deref()
            .unwrap_or("<unknown>"),
        trait_name
    );

    for t in &input.items {
        let TraitItem::Fn(f) = t else {
            impl_content.extend(
                Error::new_spanned(t, "#[extern_trait] may only contain methods")
                    .to_compile_error(),
            );
            continue;
        };

        let export_name = format!("{}_{}", symbol_prefix, f.sig.ident);

        match generate_proxy_impl(proxy_name, &export_name, &f.sig) {
            Ok(i) => {
                impl_content.extend(i);
                macro_content.extend(generate_macro_rules(None, &export_name, &f.sig));
            }
            Err(e) => {
                impl_content.extend(e.to_compile_error());
            }
        }
    }

    let mut extra_impls = TokenStream::new();

    for t in &input.supertraits {
        if let TypeParamBound::Trait(t) = t {
            if let Some(path) = t.path.get_ident() {
                if path == "Send" {
                    extra_impls.extend(quote! {
                        unsafe impl Send for #proxy_name {}
                    });
                } else if path == "Sync" {
                    extra_impls.extend(quote! {
                        unsafe impl Sync for #proxy_name {}
                    });
                }
                // TODO: support more traits
            }
        }
    }

    let drop_name = format!("{symbol_prefix}_drop");

    Ok(quote! {
        #input

        #proxy

        #unsafety impl #trait_name for #proxy_name {
            #impl_content
        }

        #extra_impls

        impl Drop for #proxy_name {
            fn drop(&mut self) {
                unsafe extern "Rust" {
                    #[link_name = #drop_name]
                    fn drop(this: *mut #proxy_name);
                }
                unsafe { drop(self) }
            }
        }

        #[doc(hidden)]
        #[macro_export]
        macro_rules! #macro_name {
            ($trait:path, $ty:ty) => {
                const _: () = {
                    #macro_content

                    #[doc(hidden)]
                    #[unsafe(export_name = #drop_name)]
                    unsafe extern "Rust" fn drop(this: &mut $ty) {
                        unsafe { ::core::ptr::drop_in_place(this) };
                    }
                };
            };
        }

        #[doc(hidden)]
        pub use #macro_name as #trait_name;
    })
}

fn generate_proxy_impl(
    proxy_name: &Ident,
    export_name: &str,
    sig: &Signature,
) -> Result<TokenStream> {
    let mut sig = sig.clone();
    let ident = &sig.ident;

    let args = sig
        .inputs
        .iter_mut()
        .enumerate()
        .map(|(i, arg)| match arg {
            FnArg::Receiver(_) => format_ident!("self"),
            FnArg::Typed(arg) => {
                let name = format_ident!("_{}", i);
                arg.pat = parse_quote!(#name);
                name
            }
        })
        .collect::<Vec<_>>();

    let mut deref = None;

    let output = match &sig.output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arr, ty) => ReturnType::Type(*arr, {
            if ty.contains_self() {
                match ty.self_kind() {
                    Some(SelfKind::Value) => parse_quote!(#proxy_name),
                    Some(SelfKind::Ptr) => parse_quote!(*const #proxy_name),
                    Some(SelfKind::Ref(mutable)) => {
                        if mutable {
                            deref = Some(quote!(&mut*));
                            parse_quote!(*mut #proxy_name)
                        } else {
                            deref = Some(quote!(&*));
                            parse_quote!(*const #proxy_name)
                        }
                    }
                    None => {
                        return Err(Error::new_spanned(
                            ty,
                            "Too complex return type for #[extern_trait]",
                        ));
                    }
                }
            } else {
                ty.clone()
            }
        }),
    };

    let inputs = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(arg) => &arg.ty,
            FnArg::Typed(arg) => &arg.ty,
        })
        .map(|ty| {
            if ty.contains_self() {
                match ty.self_kind() {
                    Some(SelfKind::Ptr) | Some(SelfKind::Ref(_)) => {
                        Ok(parse_quote!(*const #proxy_name))
                    }
                    // TODO: pass `Self` by value
                    Some(SelfKind::Value) => Err(Error::new_spanned(
                        ty,
                        "Passing `Self` by value is not supported for #[extern_trait] yet",
                    )),
                    None => Err(Error::new_spanned(
                        ty,
                        "Too complex argument type for #[extern_trait]",
                    )),
                }
            } else {
                Ok(ty.clone())
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #sig {
            unsafe extern "Rust" {
                #[link_name = #export_name]
                fn #ident(#(_: #inputs),*) #output;
            }
            unsafe { #deref #ident(#(#args),*) }
        }
    })
}

fn generate_macro_rules(trait_: Option<&Ident>, export_name: &str, sig: &Signature) -> TokenStream {
    let ident = &sig.ident;

    let output = match &sig.output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arr, ty) => ReturnType::Type(
            *arr,
            if ty.contains_self() {
                Box::new(Type::Verbatim(match ty.self_kind().unwrap() {
                    SelfKind::Value => quote!($ty),
                    SelfKind::Ptr => quote!(*const $ty),
                    SelfKind::Ref(mutable) => {
                        if mutable {
                            quote!(&mut $ty)
                        } else {
                            quote!(& $ty)
                        }
                    }
                }))
            } else {
                ty.clone()
            },
        ),
    };

    let (args, arg_tys): (Vec<_>, Vec<_>) = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(arg) => &arg.ty,
            FnArg::Typed(arg) => &arg.ty,
        })
        .enumerate()
        .map(|(i, ty)| {
            (
                format_ident!("_{}", i),
                if ty.contains_self() {
                    Box::new(Type::Verbatim(match ty.self_kind().unwrap() {
                        SelfKind::Value => quote!($ty),
                        SelfKind::Ptr => quote!(*const $ty),
                        SelfKind::Ref(mutable) => {
                            if mutable {
                                quote!(&mut $ty)
                            } else {
                                quote!(& $ty)
                            }
                        }
                    }))
                } else {
                    ty.clone()
                },
            )
        })
        .unzip();

    let trait_ = trait_.map_or_else(|| quote!($trait), |trait_| quote!(#trait_));

    quote! {
        #[doc(hidden)]
        #[unsafe(export_name = #export_name)]
        unsafe extern "Rust" fn #ident(#(#args: #arg_tys),*) #output {
            <$ty as #trait_>::#ident(#(#args),*)
        }
    }
}
