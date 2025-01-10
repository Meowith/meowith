use crate::utils::{extract_type_name, pascal_to_snake_case};
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{Fields, Type, Variant};

pub fn generate_dispatcher_struct(
    name: Ident,
    variants: Vec<Variant>,
    dispatcher_name: Ident,
    handler_name: Ident,
) -> TokenStream {
    let dispatcher_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;
        let handler_method_name = format!("handle_{}", pascal_to_snake_case(&variant.ident));
        let handler_method_name = Ident::new(&handler_method_name, proc_macro2::Span::call_site());

        match variant.fields {
            Fields::Unnamed(_) => {
                panic!("Variants need to have named arguments")
            }
            Fields::Unit => {
                quote! {
                    #name::#variant_name => {
                        self.handler.#handler_method_name(writer).await?;
                    },
                }
            }
            Fields::Named(named_args) => {
                let named = named_args.clone().named.into_iter().map(|field| field.ident);
                let mut arg_order = 0;
                let len_args = named_args.named.len();

                let deserialize_steps = named_args.named.into_iter().map(|field| {
                    let ret = match &field.ty {
                        Type::Path(type_path) => {
                            let field_ident = field.ident.clone().expect("Fields must have a name");
                            let type_name = extract_type_name(type_path);

                            match type_name.as_str() {
                                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16"
                                | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" => {
                                    let primitive_name = type_path.path.get_ident().unwrap();
                                    quote! {
                                        let size = (#primitive_name::BITS / 8) as usize;
                                        let #field_ident = #primitive_name::from_be_bytes(payload[i..i+size].try_into().unwrap());
                                        i += size;
                                    }
                                }
                                "Uuid" => quote! {
                                            let size = 16;
                                            let #field_ident = Uuid::from_slice(&payload[i..i+size])?;
                                            i += size;
                                        },
                                "Vec<u8>" => {
                                    if arg_order + 1 == len_args { // is last, just dump the data
                                        // if first and last there will be no size defined, so just dump whole payload size
                                        let size = if arg_order == 0 {
                                            quote! { let size = payload.len(); }
                                        } else {
                                            quote! { let size = (payload.len() - size) as usize; }
                                        };

                                        quote! {
                                            #size
                                            let #field_ident = Vec::from(&payload[i..i+size]);
                                        }
                                    } else {
                                        quote! {
                                            // Read length first
                                            let size = u32::from_be_bytes(payload[i..i+4].try_into().unwrap()) as usize;
                                            i += 4;
                                            let #field_ident = Vec::from(&payload[i..i+size]);
                                            i += size;
                                        }
                                    }
                                }
                                "String" => {
                                    if arg_order + 1 == len_args {
                                        // if first and last there will be no size defined, so just dump whole payload size
                                        let size = if arg_order == 0 {
                                            quote! { let size = payload.len(); }
                                        } else {
                                            quote! { let size = (payload.len() - size) as usize; }
                                        };

                                        quote! {
                                            #size
                                            let #field_ident = String::from_utf8_lossy(&payload[i..i+size]).to_string();
                                        }
                                    } else {
                                        quote! {
                                            // Read length first
                                            let size = u32::from_be_bytes(payload[i..i+4].try_into().unwrap()) as usize;
                                            i += 4;
                                            let #field_ident = String::from_utf8_lossy(&payload[i..i+size]).to_string();
                                            i += size;
                                        }
                                    }
                                }
                                _ => panic!("Unsupported datatype {:?} {:?}", type_name, type_path.path.get_ident().unwrap()),
                            }
                        }
                        _ => panic!("Bad type {:?}", &field.ty.to_token_stream().to_string()),
                    };
                    arg_order += 1;
                    ret
                });

                quote! {
                    #name::#variant_name { .. } => {
                        let mut i = 0usize;
                        #(#deserialize_steps)*
                        self.handler.#handler_method_name(writer, #(#named),*).await?;
                    }
                }
            }
        }
    });
    let struct_def = quote! {
      #[derive(Debug)]
        pub struct #dispatcher_name {
            pub handler: Box<dyn #handler_name<#name>>,
            pub writer: Weak<Mutex<PacketWriter< #name >>>,
        }
    };
    let trait_impl = quote! {

        #[async_trait]
        impl PacketDispatcher<#name> for #dispatcher_name {
            async fn dispatch_packet(
                &self,
                stream: &mut ReadHalf<TlsStream<TcpStream>>,
            ) -> ProtocolResult<()> {
                let mut header = [0u8; 5];
                stream.read_exact(&mut header).await?;

                let packet_type = #name::try_from(header[0])?;
                let payload_length = u32::from_be_bytes(header[1..5].try_into().unwrap());
                if !packet_type.validate_length(payload_length) {
                    return Err(ProtocolError::ConnectionError);
                }

                let mut payload = vec![0u8; payload_length as usize];
                stream.read_exact(&mut payload).await?;

                if let Some(writer) = self.writer.upgrade() {
                    match packet_type {
                        #(#dispatcher_arms)*
                    }
                }

                Ok(())
            }
        }
    };
    quote! {
        #struct_def

        #trait_impl
    }
}
