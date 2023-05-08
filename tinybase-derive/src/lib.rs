use core::panic;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Data, DeriveInput, Field, Fields, FieldsNamed, Ident, Meta, MetaList, Path,
};

#[proc_macro_derive(TinyBaseTable, attributes(index))]
pub fn table_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = ast.ident;

    let fields = if let Data::Struct(syn::DataStruct {
        fields: Fields::Named(FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named
    } else {
        panic!("can only derive on a struct")
    };

    let mut index_names = vec![];
    let mut index_members = vec![];

    let mut by_index = vec![];
    let mut index_initializers = vec![];
    for field in fields {
        let Field { attrs, .. } = field;

        for attr in attrs {
            if let Ok(meta) = attr.parse_meta() {
                if let Meta::Path(path) = meta {
                    // Check for index attribute.
                    if path.get_ident().unwrap()
                        != &Ident::new("index", proc_macro2::Span::call_site())
                    {
                        continue;
                    }

                    let field_name = field.ident.as_ref().unwrap();
                    let field_name_method =
                        syn::Ident::new(&format!("by_{}", field_name), field_name.span());

                    index_names.push(field_name.clone());

                    let type_name = &field.ty;
                    index_members.push(quote! {
                        #field_name: tinybase::Index<#name, #type_name>,
                    });

                    by_index.push(quote! {
                        pub fn #field_name_method(&self, #field_name: #type_name) -> tinybase::result::DbResult<Vec<tinybase::Record<#name>>> {
                            self.#field_name.select(&#field_name)
                        }
                    });

                    let field_str = format!("{}", field_name);

                    index_initializers.push(quote! {
                        let #field_name = _table.create_index(#field_str, |record| record.#field_name.clone()).unwrap();
                    })
                }
            }
        }
    }

    let vis = ast.vis.clone();
    let wrapper_name = syn::Ident::new(&format!("{}Queryable", name.to_string()), name.span());

    let expanded = quote! {
        #vis struct #wrapper_name {
            pub _table: tinybase::Table<#name>,
            pub #(#index_members)*
        }

        impl #wrapper_name {
            #(#by_index)*
        }

        impl #name {
            pub fn init(db: &tinybase::TinyBase, name: &str) -> tinybase::DbResult<#wrapper_name> {
                let _table: tinybase::Table<#name> = db.open_table(name)?;
                #(#index_initializers);*

                Ok(#wrapper_name {
                    _table, #(#index_names),*
                })
            }
        }
    };

    expanded.into()
}
