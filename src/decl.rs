use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    Error, FnArg, GenericArgument, Ident, ItemStruct, ItemTrait, PathArguments, PathSegment,
    Result, ReturnType, Signature, TraitItem, Type, TypeParamBound, parse_quote,
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
            if t.path.leading_colon.is_none() && t.path.segments.len() == 1 {
                let PathSegment { ident, arguments } = &t.path.segments[0];
                if ident == "Send" {
                    extra_impls.extend(quote! {
                        unsafe impl Send for #proxy_name {}
                    });
                } else if ident == "Sync" {
                    extra_impls.extend(quote! {
                        unsafe impl Sync for #proxy_name {}
                    });
                } else if ident == "AsRef" {
                    if let PathArguments::AngleBracketed(args) = arguments {
                        if let Some(GenericArgument::Type(ty)) = args.args.first() {
                            let export_name =
                                format!("{symbol_prefix}_AsRef_{}", ty.to_token_stream());
                            let sig = parse_quote!(fn as_ref(&self) -> &#ty);
                            let impl_content = generate_proxy_impl(proxy_name, &export_name, &sig)?;
                            extra_impls.extend(quote! {
                                impl AsRef<#ty> for #proxy_name {
                                    #impl_content
                                }
                            });
                            macro_content.extend(generate_macro_rules(
                                Some(quote!(AsRef<#ty>)),
                                &export_name,
                                &sig,
                            ));
                        }
                    }
                }
                // TODO: support more traits
            }
        }
    }

    let drop_name = format!("{symbol_prefix}_drop");
    let reflect_name = format!("{symbol_prefix}_reflect");
    let generic_doc = format!(
        "`T` must implement [`{}`] via `#[extern_trait]`.",
        trait_name
    );

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
                    safe fn drop(this: *mut #proxy_name);
                }
                drop(self)
            }
        }

        impl #proxy_name {
            unsafe fn reflect<T, R>() -> extern "Rust" fn(T) -> R {
                unsafe extern "Rust" {
                    #[link_name = #reflect_name]
                    safe fn reflect(this: #proxy_name) -> #proxy_name;
                }
                unsafe {
                    ::core::mem::transmute::<_, extern "Rust" fn(T) -> R>(reflect as *const ())
                }
            }

            /// Convert the proxy type from the implementation type.
            /// # Safety
            #[doc = #generic_doc]
            pub unsafe fn from_impl<T: #trait_name>(value: T) -> Self {
                unsafe { Self::reflect::<T, #proxy_name>()(value) }
            }

            /// Convert the proxy type into the implementation type.
            /// # Safety
            #[doc = #generic_doc]
            pub unsafe fn into_impl<T: #trait_name>(self) -> T {
                unsafe { Self::reflect::<#proxy_name, T>()(self) }
            }

            /// Returns a reference to the implementation type.
            /// # Safety
            #[doc = #generic_doc]
            pub unsafe fn downcast_ref<T: #trait_name>(&self) -> &T {
                unsafe { &*(self as *const Self as *const T) }
            }

            /// Returns a mutable reference to the implementation type.
            /// # Safety
            #[doc = #generic_doc]
            pub unsafe fn downcast_mut<T: #trait_name>(&mut self) -> &mut T {
                unsafe { &mut *(self as *mut Self as *mut T) }
            }
        }

        #[doc(hidden)]
        #[macro_export]
        macro_rules! #macro_name {
            ($trait:path: $ty:ty) => {
                const _: () = {
                    #macro_content

                    #[doc(hidden)]
                    #[unsafe(export_name = #drop_name)]
                    extern "Rust" fn drop(this: &mut $ty) {
                        unsafe { ::core::ptr::drop_in_place(this) };
                    }

                    #[doc(hidden)]
                    #[unsafe(export_name = #reflect_name)]
                    extern "Rust" fn reflect(this: $ty) -> $ty {
                        this
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

    let proxy: Box<Type> = parse_quote!(#proxy_name);

    let output = match &sig.output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arr, ty) => ReturnType::Type(*arr, {
            if ty.contains_self() {
                if let Some(kind) = ty.self_kind() {
                    kind.into_type_for(proxy.clone())
                } else {
                    return Err(Error::new_spanned(
                        ty,
                        "Too complex return type for #[extern_trait]",
                    ));
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
                if let Some(kind) = ty.self_kind() {
                    if matches!(kind, SelfKind::Value) {
                        return Err(Error::new_spanned(
                            ty,
                            "Passing `Self` by value is not supported for #[extern_trait] yet",
                        ));
                    }
                    Ok(kind.into_type_for(proxy.clone()))
                } else {
                    Err(Error::new_spanned(
                        ty,
                        "Too complex argument type for #[extern_trait]",
                    ))
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
                safe fn #ident(#(_: #inputs),*) #output;
            }
            #ident(#(#args),*)
        }
    })
}

fn generate_macro_rules(
    trait_: Option<TokenStream>,
    export_name: &str,
    sig: &Signature,
) -> TokenStream {
    let ident = &sig.ident;

    let placeholder = Box::new(Type::Verbatim(quote!($ty)));

    let output = match &sig.output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arr, ty) => ReturnType::Type(
            *arr,
            if ty.contains_self() {
                ty.self_kind().unwrap().into_type_for(placeholder.clone())
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
                    ty.self_kind().unwrap().into_type_for(placeholder.clone())
                } else {
                    ty.clone()
                },
            )
        })
        .unzip();

    let trait_ = trait_.unwrap_or_else(|| quote!($trait));

    quote! {
        #[doc(hidden)]
        #[unsafe(export_name = #export_name)]
        unsafe extern "Rust" fn #ident(#(#args: #arg_tys),*) #output {
            <$ty as #trait_>::#ident(#(#args),*)
        }
    }
}
