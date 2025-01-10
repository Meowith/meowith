use crate::utils::extract_type_name;
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{Fields, Type, Variant};

pub fn generate_validator(name: Ident, variants: Vec<Variant>) -> TokenStream {
    let validator_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        match variant.fields {
            Fields::Unnamed(_) => {
                panic!("Variants need to have named arguments")
            }
            Fields::Unit => {
                quote! {
                    #name::#variant_name => {
                        len == 0
                    }
                }
            }
            Fields::Named(named_args) => {
                let mut arg_order = 0;
                let mut is_var: bool = false;
                let mut packet_size: u32 = 0;
                let len_args = named_args.named.len();
                named_args
                    .named
                    .into_iter()
                    .for_each(|field| match &field.ty {
                        Type::Path(type_path) => {
                            let type_name = extract_type_name(type_path);

                            match type_name.as_str() {
                                "i8" | "u8" => {
                                    packet_size += 1;
                                }
                                "i16" | "u16" => {
                                    packet_size += 2;
                                }
                                "i32" | "u32" => {
                                    packet_size += 4;
                                }
                                "i64" | "u64" => {
                                    packet_size += 8;
                                }
                                "i128" | "u128" | "Uuid" => {
                                    packet_size += 16;
                                }
                                "Vec<u8>" => {
                                    is_var = true;
                                    if arg_order + 1 == len_args {
                                        // if last, just dump the data, no min length change
                                    } else {
                                        packet_size += 4;
                                    }
                                }
                                "String" => {
                                    is_var = true;
                                    if arg_order + 1 == len_args {
                                        // if last, just dump the data, no min length change
                                    } else {
                                        packet_size += 4;
                                    }
                                }
                                _ => panic!(
                                    "Unsupported datatype {:?} {:?}",
                                    type_name,
                                    type_path.path.get_ident().unwrap().to_string()
                                ),
                            };
                            arg_order += 1;
                        }
                        _ => panic!("Bad type {:?}", &field.ty.to_token_stream().to_string()),
                    });

                if is_var {
                    quote! {
                        #name::#variant_name { .. } => {
                            len >= #packet_size
                        }
                    }
                } else {
                    quote! {
                        #name::#variant_name { .. } => {
                            len == #packet_size
                        }
                    }
                }
            }
        }
    });

    quote! {
        impl Packet for #name {
            fn validate_length(&self, len: u32) -> bool {
                match &self {
                    #(#validator_arms)*
                }
            }
        }
    }
}
