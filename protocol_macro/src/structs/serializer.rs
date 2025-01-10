use crate::utils::extract_type_name;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Fields, Type, Variant};

pub fn generate_serializer_struct(
    name: Ident,
    variants: Vec<Variant>,
    serializer_name: Ident,
) -> TokenStream {
    let serializer_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        match variant.fields {
            Fields::Unnamed(_) => {
                panic!("Variants need to have named arguments")
            }
            Fields::Unit => {
                quote! {
                    #name::#variant_name => {
                        let mut res: Vec<u8> = Vec::new();
                        res.push(id);
                        res.extend_from_slice(&[0u8; 4]);
                        res
                    }
                }
            }
            Fields::Named(named_args) => {
                let len_args = named_args.named.len();

                // Building enum variant fields list
                let variant_body = named_args.named.clone().into_iter()
                    .map(|field| {
                        let field_ident = field.ident.clone().expect("Fields must have a name");

                        match &field.ty {
                            Type::Path(type_path) => {
                                let type_name = extract_type_name(type_path);
                                match type_name.as_str() {
                                    // if type is Vec<u8> we have to specify mutability
                                    "Vec<u8>" => { quote! { mut #field_ident } }
                                    _ => { quote! { #field_ident } }
                                }
                            }
                            _ => panic!("Bad type {:?}", &field.ty),
                        }
                    });

                let mut arg_order = 0;
                let serialize_steps = named_args.named.into_iter().map(|field| match &field.ty {
                    Type::Path(type_path) => {
                        let field_ident = field.ident.clone().expect("Fields must have a name");
                        let type_name = extract_type_name(type_path);

                        let res = match type_name.as_str() {
                            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16"
                            | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" => {
                                quote! { res.extend_from_slice(&#field_ident.to_be_bytes()); }
                            }
                            "Uuid" => {
                                quote! { res.extend_from_slice(#field_ident.as_bytes().as_slice()); }
                            }
                            "Vec<u8>" => {
                                if arg_order + 1 == len_args {
                                    // if last, just dump the data
                                    quote! {
                                        res.append(&mut #field_ident);
                                    }
                                } else {
                                    quote! {
                                        res.extend_from_slice(&(#field_ident.len() as u32).to_be_bytes());
                                        res.append(&mut #field_ident);
                                    }
                                }
                            }
                            "String" => {
                                if arg_order + 1 == len_args {
                                    // if last, just dump the data
                                    quote! {
                                        for byte in #field_ident.as_bytes() { res.push(&byte); }
                                    }
                                } else {
                                    quote! {
                                        res.extend_from_slice(&(#field_ident.len() as u32).to_be_bytes());
                                        for byte in #field_ident.as_bytes() {
                                            res.push(*byte);
                                        }
                                    }
                                }
                            }
                            _ => panic!("Unsupported datatype {:?} {:?}", type_name, type_path),
                        };
                        arg_order += 1;
                        res
                    }
                    _ => panic!("Bad type {:?}", &field.ty),
                });

                quote! {
                    #name::#variant_name { #(#variant_body),* } => {
                        let mut res: Vec<u8> = Vec::new();
                        res.push(id);
                        res.extend_from_slice(&[0u8; 4]); // yet empty size field

                        #(#serialize_steps)*

                        let payload_length = res.len() - PROTOCOL_HEADER_SIZE;
                        res[1 .. 5].copy_from_slice(&payload_length.to_be_bytes());

                        res
                    }
                }
            }
        }
    });
    let struct_def = quote! {
        #[derive(Debug)]
        struct #serializer_name;
    };
    let trait_impl = quote! {
        #[async_trait]
        impl PacketSerializer<#name> for #serializer_name {
            fn serialize_packet(&self, packet: #name) -> Vec<u8> {
                let id = u8::from(&packet);
                match packet {
                    #(#serializer_arms)*
                }
            }
        }
    };

    quote! {
        #struct_def
        #trait_impl
    }
}
