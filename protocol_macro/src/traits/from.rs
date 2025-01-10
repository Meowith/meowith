use crate::utils::extract_type_name;
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{Type, Variant};

pub fn default_value(ty: &Type) -> TokenStream {
    match ty {
        Type::Path(type_path) => {
            let type_name = extract_type_name(type_path);

            match type_name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
                | "u128" | "usize" | "f32" | "f64" => quote! { 0 },
                "Uuid" => quote! { Uuid::nil() },
                "Vec<u8>" => quote! { Vec::new() },
                "String" => quote! { String::new() },
                _ => panic!("Unsupported datatype {:?} {:?}", type_name, type_path.path.get_ident().unwrap().to_string()),
            }
        }
        _ => panic!("Bad type {:?}", ty.to_token_stream().to_string()),
    }
}

pub fn generate_from(name: Ident, variants: Vec<Variant>) -> TokenStream {
    let mut i: u8 = 0;
    let try_from_u8_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;
        let variant_fields = variant.fields.into_iter().map(|field| {
            let field_name = field.ident.expect("Fields must have a name");
            let default_val = default_value(&field.ty);
            quote! { #field_name: #default_val }
        });
        i += 1;
        quote! {
            #i => Ok(#name::#variant_name {
                  #(#variant_fields),*
            }),
        }
    });

    let mut i: u8 = 0;

    let from_self_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        i += 1;
        quote! {
            #name::#variant_name { .. } => #i,
        }
    });

    quote! {
        impl TryFrom<u8> for #name {
            type Error = ProtocolError;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    #(#try_from_u8_arms)*
                    _ => Err(ProtocolError::InvalidFormat)
                }
            }
        }
        impl From<&#name> for u8 {
            fn from(value: &#name) -> u8 {
                match value {
                    #(#from_self_arms)*
                }
            }
        }
    }
}
