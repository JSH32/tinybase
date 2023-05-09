use proc_macro2::{Ident, TokenStream};
use syn::{Attribute, Meta};

/// This returns the attribute [`Ident`] if the attribute was found.
pub fn has_attribute(attrs: &Vec<Attribute>, attr_name: &str) -> Option<(Ident, Meta)> {
    for attr in attrs {
        if let Ok(meta) = attr.parse_meta() {
            let path = meta.path();
            if let Some(ident) = path.get_ident() {
                if ident == attr_name {
                    return Some((ident.clone(), meta.clone()));
                }
            }
        }
    }

    None
}

/// Get a value in an attribute.
pub fn get_list_attr(
    attrs: &Vec<Attribute>,
    attr_name: &str,
) -> Result<Vec<TokenStream>, TokenStream> {
    let mut matches = vec![];

    for attr in attrs {
        let meta = attr.parse_meta().map_err(|err| err.to_compile_error())?;
        if let syn::Meta::List(path) = meta {
            if let Some(ident) = path.path.get_ident() {
                if ident == attr_name {
                    let tokens = attr.parse_args().map_err(|err| err.to_compile_error())?;
                    matches.push(tokens);
                }
            }
        }
    }

    Ok(matches)
}

/// Make sure the state of attributes is allowed.
/// This returns the attribute [`Ident`] of the relevant span when validation failed.
pub fn validate_attributes(
    attrs: &Vec<Attribute>,
    base: Option<&str>,
    other: &[(&str, bool)],
    illegal: &[&str],
) -> Result<(), TokenStream> {
    for attr in illegal {
        if let Some(ident) = has_attribute(attrs, attr) {
            return Err(
                syn::Error::new(ident.0.span(), "This attribute is not allowed here")
                    .to_compile_error()
                    .into(),
            );
        }
    }

    for attr in other {
        let found = has_attribute(attrs, attr.0);
        if let Some(found) = found {
            if let Some(base) = base {
                if !has_attribute(attrs, base).is_some() {
                    return Err(syn::Error::new(
                        found.0.span(),
                        format!("This attribute requires the #[{}] attribute", base),
                    )
                    .to_compile_error()
                    .into());
                }
            }

            match found.1 {
                Meta::Path(_) => {
                    if attr.1 {
                        return Err(syn::Error::new(
                            found.0.span(),
                            "This attribute is missing a parameter",
                        )
                        .to_compile_error()
                        .into());
                    }
                }
                Meta::List(_) => {
                    if !attr.1 {
                        return Err(
                            syn::Error::new(found.0.span(), "This attribute isn't a list")
                                .to_compile_error()
                                .into(),
                        );
                    }
                }
                _ => unimplemented!(),
            };
        }
    }

    Ok(())
}
