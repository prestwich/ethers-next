use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, FieldsNamed, FieldsUnnamed};

#[proc_macro_derive(SolAbiType, attributes(abi_skip))]
pub fn encode_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input);
    impl_tokenize(&ast).into()
}

fn impl_tokenize(ast: &syn::DeriveInput) -> TokenStream {
    let primary_type = &ast.ident;
    let pushes = tokenize_fields(ast);
    quote! {
        impl ::ethers_abi_enc::Tokenize for #primary_type {
            fn to_token(&self) -> ::ethers_abi_enc::Token {
                let mut tokens = Vec::new();
                #pushes
                Token::FixedSeq(tokens)
            }
        }
    }
}

fn tokenize_fields(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    if let Data::Struct(data) = &ast.data {
        match &data.fields {
            syn::Fields::Named(fields) => tokenize_named_fields(fields),
            syn::Fields::Unnamed(fields) => tokenize_unnamed_fields(fields),
            syn::Fields::Unit => {
                panic!("cannot ABI encode the unit type. Please abi_skip this field")
            }
        }
    } else {
        panic!("Struct must contain at least 1 field")
    }
}

fn tokenize_named_fields(fields: &FieldsNamed) -> proc_macro2::TokenStream {
    fields
        .named
        .iter()
        .filter(|f| !f.attrs.iter().any(|attr| attr.path.is_ident("abi_skip")))
        .map(|f| {
            let field_name = f.ident.as_ref().unwrap();
            quote! {
                tokens.push(self.#field_name.to_token());
            }
        })
        .collect()
}

fn tokenize_unnamed_fields(fields: &FieldsUnnamed) -> proc_macro2::TokenStream {
    fields
        .unnamed
        .iter()
        .filter(|f| !f.attrs.iter().any(|attr| attr.path.is_ident("abi_skip")))
        .enumerate()
        .map(|(field_num, _f)| {
            let field_num = syn::Index::from(field_num);
            quote! {
                let t = ::ethers_abi_enc::Tokenize::to_token(&self.#field_num);
                tokens.push(t);
            }
        })
        .collect()
}
