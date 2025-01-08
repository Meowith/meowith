use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type, TypePath, Variant};

fn pascal_to_snake_case(ident: &Ident) -> String {
    let pascal_case = ident.to_string();
    let snake_case = pascal_case
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("_{}", c.to_lowercase())
            } else {
                c.to_string()
            }
        })
        .collect::<String>()
        .to_lowercase();
    snake_case
}

fn extract_type_name(path: &TypePath) -> String {
    path.path
        .segments
        .iter()
        .map(|segment| {
            let mut result = segment.ident.to_string();

            if let syn::PathArguments::AngleBracketed(ref args) = segment.arguments {
                let args_str = args
                    .args
                    .iter()
                    .filter_map(|arg| {
                        if let syn::GenericArgument::Type(Type::Path(type_path)) = arg {
                            Some(
                                type_path
                                    .path
                                    .segments
                                    .iter()
                                    .map(|seg| seg.ident.to_string())
                                    .collect::<Vec<_>>()
                                    .join("::"),
                            )
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                result = format!("{}<{}>", result, args_str);
            }
            result
        })
        .collect::<Vec<_>>()
        .join("::")
}

/// Derives the necessary structs and traits for a Meowith protocol
///
/// Packet structure:
///
/// | u8 packet id | u32 payload size | payload ... |
#[proc_macro_derive(Protocol)]
pub fn derive_protocol(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants: Vec<Variant> = if let Data::Enum(data_enum) = &input.data {
        data_enum
            .clone()
            .variants
            .into_iter()
            .filter_map(|variant| match &variant.fields {
                Fields::Named(_) => Some(variant),
                Fields::Unit => Some(variant),
                _ => None,
            })
            .collect()
    } else {
        panic!("Protocol trait can only be derived for enums");
    };

    let dispatcher_name = format!("{}{}", name, "Dispatcher");
    let dispatcher_name = Ident::new(&dispatcher_name, name.span());

    let serializer_name = format!("{}{}", name, "Serializer");
    let serializer_name = Ident::new(&serializer_name, name.span());

    let handler_name = format!("{}{}", name, "Handler");
    let handler_name = Ident::new(&handler_name, name.span());

    let serializer_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        match variant.fields {
            Fields::Unnamed(_) => {
                panic!("Variants need to have named arguments")
            }
            Fields::Unit => {
                quote! {
                    #name::#variant_name => {
                        let res: Vec<u8> = Vec::new();
                        let id: u8 = #name::#variant_name.into();
                        res.push(id);
                        res.push(&[0u8; 4]);
                        res
                    }
                }
            }
            Fields::Named(named_args) => {
                let named = named_args.clone().named.into_iter();

                let mut arg_order = 0;
                let len_args = named_args.named.len();
                let serialize_steps = named_args.named.into_iter().map(|field| match &field.ty {
                    Type::Path(type_path) => {
                        let field_ident = field.ident.clone().expect("Fields must have a name");
                        let type_name = extract_type_name(type_path);

                        let res = match type_name.as_str() {
                            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16"
                            | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" => {
                                quote! { res.push(&#field_ident.to_be_bytes()); }
                            }
                            "Uuid" => {
                                quote! { res.push(#field_ident.as_bytes().as_slice()); }
                            }
                            "Vec<u8>" => {
                                if arg_order + 1 == len_args {
                                    // if last, just dump the data
                                    quote! {
                                        res.append(&mut #field_ident);
                                    }
                                } else {
                                    quote! {
                                        res.push(&(#field_ident.len() as u32).to_be_bytes());
                                        res.append(&mut #field_ident);
                                    }
                                }
                            }
                            "String" => {
                                if arg_order + 1 == len_args {
                                    // if last, just dump the data
                                    quote! {
                                        for byte in #field_ident.as_bytes() {
                                            res.push(&byte);
                                        }
                                    }
                                } else {
                                    quote! {
                                        res.push(&(#field_ident.len() as u32).to_be_bytes());
                                        for byte in #field_ident.as_bytes() {
                                            res.push(&byte);
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
                    #name::#variant_name { #(#named),* } => {
                        let res: Vec<u8> = Vec::new();
                        let id: u8 = #name::#variant_name.into();
                        res.push(id);
                        res.push(&[0u8; 4]); // yet empty size field

                        #(#serialize_steps)*

                        let payload_length = res.len() - PROTOCOL_HEADER_SIZE;
                        res[1 .. 5].copy_from_slice(&payload_length.to_be_bytes());

                        res
                    }
                }
            }
        }
    });

    let serializer = quote! {
        struct #serializer_name {};

        impl PacketSerializer<#name> for #serializer_name {
            fn serialize_packet(&self, packet: #name) -> Vec<u8> {
                match self {
                    #(#serializer_arms)*
                }
            }
        }
    };

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
                                _ => panic!("Unsupported datatype {:?} {:?}", type_name, type_path),
                            };
                            arg_order += 1;
                            
                        }
                        _ => panic!("Bad type {:?}", &field.ty),
                    });

                if is_var {
                    quote! {
                        #name::#variant_name { .. } => {
                            len > #packet_size
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

    let validator = quote! {
        impl Packet for #name {
            pub fn validate_length(&self, len: u32) -> bool {
                match &self {
                    #(#validator_arms)*
                }
            }
        }
    };

    let handler_type = {
        let handler_methods = variants.clone().into_iter().map(|variant| {
                let handler_method_name = format!("handle_{}", pascal_to_snake_case(&variant.ident));
                let handler_method_name = Ident::new(&handler_method_name, proc_macro2::Span::call_site());

                match variant.fields {
                    Fields::Named(named_args) => {
                        let named = named_args.named.into_iter();
                        quote! {
                        async fn #handler_method_name(writer: Arc<Mutex<PacketWriter<T>>>, #(#named),*) -> Result<(), ProtocolError>;
                    }
                    }
                    Fields::Unnamed(_) => panic!("Unnamed fields are not supported"),
                    Fields::Unit => {
                        quote! {
                        async fn #handler_method_name(writer: Arc<Mutex<PacketWriter<T>>>) -> Result<(), ProtocolError>;
                    }
                    }
                }
            });

        quote! {
            #[async_trait]
            trait #handler_name<T: Packet + 'static + Send> {
                #(
                    #handler_methods
                )*
            }
        }
    };

    let mut i = 0;
    let try_from_u8_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        i += 1;
        quote! {
            #i => Ok(#name::#variant_name),
        }
    });

    let mut i = 0;

    let from_self_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        i += 1;
        quote! {
            #name::#variant_name => #i,
        }
    });

    let u8_conversion = quote! {
        impl TryFrom<u8> for #name {
            type Error = ();

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    #(#try_from_u8_arms)*
                    _ => Err(())
                }
            }
        }
        impl From<Self> for u8 {
            fn from(value: Self) -> Self {
                match value {
                    #(#from_self_arms)*
                }
            }
        }
    };

    let dispatcher = {
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
                            self.handler.#handler_method_name(writer);
                        },
                    }
                    }
                    Fields::Named(named_args) => {
                        let named = named_args.clone().named.into_iter();
                        let named2 = named.clone().map(|arg| arg.ident.unwrap());
                        let mut arg_order = 0;
                        let len_args = named_args.named.len();

                        let deserialize_steps = named_args.named.into_iter().map(|field| {
                            let ret = match &field.ty {
                                Type::Path(type_path) => {
                                    let field_ident =
                                        field.ident.clone().expect("Fields must have a name");
                                    let type_name = extract_type_name(type_path);

                                    match type_name.as_str() {
                                        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16"
                                        | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" => {
                                            let primitive_name = type_path.path.get_ident().unwrap();
                                            quote! {
                                                let size = #primitive_name::BITS / 8;
                                                let #field_ident = #primitive_name::from_be_bytes(packet.payload[i..i+size].try_into().unwrap());
                                                i += size;
                                            }
                                        }
                                        "Uuid" => {
                                            quote! {
                                            let size = 16;
                                            let #field_ident = Uuid::from_slice(&packet.payload[i..i+size])?;
                                            i += size;
                                        }
                                        }
                                        "Vec<u8>" => {
                                            if arg_order + 1 == len_args { // is last, just dump the data
                                                quote! {
                                                    let size = packet.payload.len() - size;
                                                    let #field_ident = Vec::from(&packet.payload[i..i+size]);
                                                }
                                            } else {
                                                quote! {
                                                    // Read length first
                                                    let size = u32::from_be_bytes(packet.payload[i..i+4].try_into().unwrap());
                                                    i += 4;
                                                    let #field_ident = Vec::from(&packet.payload[i..i+size]);
                                                    i += size;
                                                }
                                            }
                                        }
                                        "String" => {
                                            if arg_order + 1 == len_args {
                                                quote! {
                                                    let size = packet.payload.len() - size;
                                                    let #field_ident = String::from_utf8_lossy(&packet.payload[i..i+size]).as_str().to_string();
                                                }
                                            } else {
                                                quote! {
                                                    // Read length first
                                                    let size = u32::from_be_bytes(packet.payload[i..i+4].try_into().unwrap());
                                                    i += 4;
                                                    let #field_ident = String::from_utf8_lossy(&packet.payload[i..i+size]).as_str().to_string();
                                                    i += size;
                                                }
                                            }
                                        }
                                        _ => panic!("Unsupported datatype {:?} {:?}", type_name, type_path),
                                    }
                                }
                                _ => panic!("Bad type {:?}", &field.ty),
                            };
                            arg_order += 1;
                            ret
                        });

                        quote! {
                        #name::#variant_name { #(#named),* } => {
                            let i = 0usize;
                            #(#deserialize_steps)*
                            self.handler.#handler_method_name(writer, #(#named2),*)?;
                        }
                    }
                    }
                }
            });

        quote! {
            struct #dispatcher_name {
                handler: #dispatcher_name,
                writer: Weak<Mutex<PacketWriter<T>>>,
            };

            impl PacketDispatcher<#name> for #dispatcher_name {
                async fn dispatch_packet(
                    &self,
                    stream: &mut ReadHalf<TlsStream<TcpStream>>,
                ) -> Result<(), ProtocolError> {
                    let header = [0u8; 5];
                    stream.read_exact(&mut header)?;

                    let packet_type = #name::try_from(header[0])?;
                    let payload_length = u32::from_be_bytes(header[1..5].try_into().unwrap());
                    if !packet_type.validate_length(payload_length) {
                        return Err(ProtocolError::ConnectionError);
                    }

                    let mut payload = vec![0u8; header.payload_size as usize];
                    stream.read_exact(&mut payload)?;

                    if let Some(writer) = self.writer.upgrade() {
                        match packet_type {
                            #(#dispatcher_arms)*
                        }
                    }

                    Ok(())
                }
            }
        }
    };

    let expanded = quote! {
        #validator

        #handler_type

        #u8_conversion

        #serializer

        #dispatcher
    };

    TokenStream::from(expanded)
}
