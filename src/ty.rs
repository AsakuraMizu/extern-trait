use syn::{
    GenericArgument, Lifetime, PathArguments, ReturnType, Token, Type, TypePtr, TypeReference,
    parse_quote,
};

#[derive(Debug, Clone, Copy)]
pub enum SelfKind<'a> {
    Value,
    Ptr {
        star_token: &'a Token![*],
        const_token: &'a Option<Token![const]>,
        mutability: &'a Option<Token![mut]>,
    },
    Ref {
        and_token: &'a Token![&],
        lifetime: &'a Option<Lifetime>,
        mutability: &'a Option<Token![mut]>,
    },
}

impl SelfKind<'_> {
    pub fn into_type_for(self, elem: Box<Type>) -> Box<Type> {
        match self {
            SelfKind::Value => elem,
            SelfKind::Ptr {
                star_token,
                const_token,
                mutability,
            } => Box::new(Type::Ptr(TypePtr {
                star_token: *star_token,
                const_token: *const_token,
                mutability: *mutability,
                elem,
            })),
            SelfKind::Ref {
                and_token,
                lifetime,
                mutability,
            } => Box::new(Type::Reference(TypeReference {
                and_token: *and_token,
                lifetime: lifetime.clone(),
                mutability: *mutability,
                elem,
            })),
        }
    }
}

pub trait TypeExt {
    fn contains_self(&self) -> bool;
    fn self_kind(&self) -> Option<SelfKind>;
}

impl TypeExt for Type {
    fn contains_self(&self) -> bool {
        match self {
            Type::Array(arr) => arr.elem.contains_self(),
            Type::BareFn(f) => {
                for arg in &f.inputs {
                    if arg.ty.contains_self() {
                        return true;
                    }
                }
                if let ReturnType::Type(_, ret) = &f.output {
                    if ret.contains_self() {
                        return true;
                    }
                }
                false
            }
            Type::Group(group) => group.elem.contains_self(),
            Type::Paren(paren) => paren.elem.contains_self(),
            Type::Path(path) => {
                if let Some(qself) = &path.qself {
                    if qself.ty.contains_self() {
                        return true;
                    }
                }
                for segment in &path.path.segments {
                    if segment.ident == "Self" {
                        return true;
                    }
                    match &segment.arguments {
                        PathArguments::None => {}
                        PathArguments::AngleBracketed(args) => {
                            for arg in &args.args {
                                if let GenericArgument::Type(ty) = arg {
                                    if ty.contains_self() {
                                        return true;
                                    }
                                }
                            }
                        }
                        PathArguments::Parenthesized(args) => {
                            for arg in &args.inputs {
                                if arg.contains_self() {
                                    return true;
                                }
                            }
                            if let ReturnType::Type(_, ret) = &args.output {
                                if ret.contains_self() {
                                    return true;
                                }
                            }
                        }
                    }
                }
                false
            }
            Type::Ptr(ptr) => ptr.elem.contains_self(),
            Type::Reference(r) => r.elem.contains_self(),
            Type::Slice(slice) => slice.elem.contains_self(),
            Type::Tuple(tpl) => {
                for elem in &tpl.elems {
                    if elem.contains_self() {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn self_kind(&self) -> Option<SelfKind> {
        let self_ty = parse_quote!(Self);

        if *self == self_ty {
            Some(SelfKind::Value)
        } else if let Type::Ptr(TypePtr {
            star_token,
            const_token,
            mutability,
            elem,
        }) = self
        {
            if **elem == self_ty {
                Some(SelfKind::Ptr {
                    star_token,
                    const_token,
                    mutability,
                })
            } else {
                None
            }
        } else if let Type::Reference(TypeReference {
            and_token,
            lifetime,
            mutability,
            elem,
        }) = self
        {
            if **elem == self_ty {
                Some(SelfKind::Ref {
                    and_token,
                    lifetime,
                    mutability,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
