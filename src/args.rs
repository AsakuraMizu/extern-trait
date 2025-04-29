use syn::{
    Attribute, Fields, Ident, ItemStruct, Result, Visibility,
    parse::{Parse, ParseStream},
    parse_quote,
};

pub struct Proxy {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub ident: Ident,
}

impl Parse for Proxy {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;

        Ok(Proxy { attrs, vis, ident })
    }
}

impl From<Proxy> for ItemStruct {
    fn from(value: Proxy) -> Self {
        let Proxy { attrs, vis, ident } = value;

        ItemStruct {
            attrs,
            vis,
            struct_token: Default::default(),
            ident,
            generics: Default::default(),
            fields: Fields::Unnamed(parse_quote!((*const (), *const ()))),
            semi_token: None,
        }
    }
}
