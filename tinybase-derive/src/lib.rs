mod utils;
use core::panic;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, FieldsNamed, Ident};
use utils::{get_list_attr, has_attribute, validate_attributes};

#[proc_macro_derive(Repository, attributes(index, unique, check))]
pub fn repository(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = ast.ident;

    let fields = match ast.data {
        Data::Struct(syn::DataStruct {
            fields: Fields::Named(FieldsNamed { ref named, .. }),
            ..
        }) => named,
        _ => panic!("can only derive on a struct"),
    };

    let (index_names, index_members, by_index, index_initializers) =
        match process_fields(&name, fields.iter()) {
            Ok(v) => v,
            Err(e) => return e,
        };

    if let Err(tokens) =
        validate_attributes(&ast.attrs, None, &[("check", true)], &["unique", "index"])
    {
        return tokens.into();
    }

    let checks: Vec<proc_macro2::TokenStream> = match get_list_attr(&ast.attrs, "check") {
        Ok(v) => v,
        Err(err) => return err.into(),
    }
    .iter()
    .map(|check_fn| {
        return quote! {
            _table.constraint(tinybase::Constraint::check(#check_fn))?;
        };
    })
    .collect();

    let vis = ast.vis.clone();
    let wrapper_name = syn::Ident::new(&format!("{}Repository", name.to_string()), name.span());

    let expanded = quote! {
        #[derive(Clone)]
        #vis struct #wrapper_name {
            _table: tinybase::Table<#name>,
            #(#index_members)*
        }

        impl std::ops::Deref for #wrapper_name {
            type Target = tinybase::Table<#name>;

            fn deref(&self) -> &Self::Target {
                &self._table
            }
        }

        impl #wrapper_name {
            #(#by_index)*
        }

        impl #name {
            pub fn init(db: &tinybase::TinyBase, name: &str) -> tinybase::DbResult<#wrapper_name> {
                let _table: tinybase::Table<#name> = db.open_table(name)?;
                #(#index_initializers);*
                #(#checks)*

                Ok(#wrapper_name {
                    _table, #(#index_names),*
                })
            }
        }
    };

    expanded.into()
}

/// Process fields and decide what should be generated for each field.
fn process_fields<'a>(
    struct_name: &proc_macro2::Ident,
    fields: impl Iterator<Item = &'a Field>,
) -> Result<
    (
        Vec<Ident>,
        Vec<proc_macro2::TokenStream>,
        Vec<proc_macro2::TokenStream>,
        Vec<proc_macro2::TokenStream>,
    ),
    TokenStream,
> {
    let mut index_names = vec![];
    let mut index_members = vec![];

    let mut by_index = vec![];
    let mut index_initializers = vec![];

    for field in fields {
        validate_attributes(
            &field.attrs,
            Some("index"),
            &[("unique", false), ("index", false)], // index is here as a hack to prevent allowing list.
            &["check"],
        )?;

        if has_attribute(&field.attrs, "index").is_some() {
            let (field_name, type_name) = (field.ident.as_ref().unwrap(), &field.ty);

            index_names.push(field_name.clone());

            index_members.push(quote! {
                pub #field_name: tinybase::Index<#struct_name, #type_name>,
            });

            let methods = create_methods(field_name, type_name, struct_name);

            by_index.push(methods);

            let field_str = format!("{}", field_name);

            index_initializers.push(quote! {
                let #field_name = _table.create_index(#field_str, |record| record.#field_name.clone())?;
            });

            if has_attribute(&field.attrs, "unique").is_some() {
                index_initializers.push(quote! {
                    _table.constraint(tinybase::Constraint::unique(&#field_name))?;
                })
            }
        }
    }

    Ok((index_names, index_members, by_index, index_initializers))
}

/// Create methods for an index.
fn create_methods(
    field_name: &Ident,
    type_name: &syn::Type,
    name: &Ident,
) -> proc_macro2::TokenStream {
    let find_method = syn::Ident::new(&format!("find_by_{}", field_name), field_name.span());
    let delete_method = syn::Ident::new(&format!("delete_by_{}", field_name), field_name.span());
    let update_method = syn::Ident::new(&format!("update_by_{}", field_name), field_name.span());

    quote! {
        pub fn #find_method(&self, #field_name: #type_name) -> tinybase::result::DbResult<Vec<tinybase::Record<#name>>> {
            self.#field_name.select(&#field_name)
        }

        pub fn #delete_method(&self, #field_name: #type_name) -> tinybase::result::DbResult<Vec<tinybase::Record<#name>>> {
            self.#field_name.delete(&#field_name)
        }

        pub fn #update_method(&self, #field_name: #type_name, updater: fn(#name) -> #name) -> tinybase::result::DbResult<Vec<tinybase::Record<#name>>> {
            let records: Vec<u64> = self.#field_name.select(&#field_name)?.iter().map(|r| r.id).collect();
            self._table.update(&records, updater)
        }
    }
}
