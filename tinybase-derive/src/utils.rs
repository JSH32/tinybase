use proc_macro2::Ident;
use syn::Field;

/// This returns the attribute [`Ident`] if the attribute was found.
pub fn has_attribute(param: &Field, attr_name: &str) -> Option<Ident> {
    for attr in &param.attrs {
        if let Ok(meta) = attr.parse_meta() {
            if let syn::Meta::Path(path) = meta {
                if let Some(ident) = path.get_ident() {
                    if ident == attr_name {
                        return Some(ident.clone());
                    }
                }
            }
        }
    }

    None
}

/// Make sure the state of attributes is allowed.
/// This returns the attribute [`Ident`] of the relevant span when validation failed.
pub fn validate_attributes(param: &Field, base: &str, other: &[&str]) -> Option<Ident> {
    if has_attribute(param, base).is_some() {
        None
    } else {
        for attr in other {
            let found = has_attribute(param, attr);
            if found.is_some() {
                return found;
            }
        }

        None
    }
}
