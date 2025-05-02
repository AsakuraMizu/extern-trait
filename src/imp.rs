use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{ItemImpl, Result, spanned::Spanned};

pub fn expand(input: ItemImpl) -> Result<TokenStream> {
    let Some((_, trait_, _)) = &input.trait_ else {
        return Err(syn::Error::new(Span::call_site(), "expected a trait impl"));
    };

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input.generics,
            "#[extern_trait] impls may not have generics",
        ));
    }

    if let Some(where_clause) = &input.generics.where_clause {
        return Err(syn::Error::new_spanned(
            where_clause,
            "#[extern_trait] impls may not have a where clause",
        ));
    }

    let ty = &input.self_ty;

    let assert_size = quote_spanned! {ty.span()=>
        const _: () = {
            assert!(
                ::core::mem::size_of::<#ty>() <= ::core::mem::size_of::<usize>() * 2,
                concat!(stringify!(#ty), " is too large to be used with #[extern_trait]")
            );
        };
    };

    Ok(quote! {
        #input

        #assert_size

        #trait_!(#trait_: #ty);
    })
}
