use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type, Variant};

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

const PROTOCOL_HEADER_SIZE: usize = 5;

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
            .filter_map(|variant| {
                if let Fields::Named(_) = &variant.fields {
                    Some(variant)
                } else if let Fields::Unit = &variant.fields {
                    Some(variant)
                } else {
                    None
                }
            })
            .collect()
    } else {
        panic!("MyMacro can only be derived for enums");
    };

    let parser_name = format!("{}{}", name, "Parser");
    let parser_name = Ident::new(&parser_name, name.span());

    let builder_name = format!("{}{}", name, "Builder");
    let builder_name = Ident::new(&builder_name, name.span());

    let handler_name = format!("{}{}", name, "Handler");
    let handler_name = Ident::new(&handler_name, name.span());

    // TODO Derives:
    // Parser trait
    // Protocol trait
    // TODO errors
    // TODO consider a streamId field in header
    // TODO handle arbitrary size payloads (string and Vec<u8>)
    // note: we can force them to be at the end of the payload
    // thus skipping the need to encode the length of each such field before it

    let builder_arms = variants.clone().into_iter().map(|variant| {
        let variant_name = &variant.ident;

        match variant.fields {
            Fields::Unnamed(_) => {
                panic!("Variants need to have named arguments")
            }
            Fields::Unit => {
                quote! {
                    #name::#variant_name => Vec::new(),
                }
            }
            Fields::Named(named_args) => {
                let named = named_args.clone().named.into_iter();

                let serialize_steps = named_args.named.into_iter().map(|field| match &field.ty {
                    Type::Path(type_path) => {
                        let field_ident = field.ident.clone().expect("Fields must have a name");
                        let type_name = type_path.path.get_ident().map(|ident| ident.to_string());
                        match type_name.as_deref() {
                            Some("i8") | Some("i16") | Some("i32") | Some("i64") | Some("i128")
                            | Some("isize") | Some("u8") | Some("u16") | Some("u32")
                            | Some("u64") | Some("u128") | Some("usize") | Some("f32")
                            | Some("f64") => {
                                quote! { res.push(&#field_ident.to_be_bytes()); }
                            }
                            Some("Uuid") => {
                                quote! { res.push(#field_ident.as_bytes().as_slice()); }
                            }
                            _ => panic!("Unsupported datatype {:?}", &type_path.path),
                        }
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

    let builder = quote! {
        struct #builder_name {};

        impl PacketBuilder<#name> for #builder_name {
            fn build_packet<T>(&self, packet: T) -> Vec<u8> {
                match self {
                    #(#builder_arms)*
                }
            }
        }
    };

    // TODO: akin to the mdsftp handler,
    // it should pass it some sort of a channel and return some sort of Result<(), SomeError>
    let handler_type = {
        let handler_methods = variants.clone().into_iter().map(|variant| {
            let handler_name = format!("handler_{}", pascal_to_snake_case(&variant.ident));
            let handler_name = Ident::new(&*handler_name, proc_macro2::Span::call_site());

            match variant.fields {
                Fields::Named(named_args) => {
                    let named = named_args.named.into_iter();
                    quote! {
                        pub async fn #handler_name(#(#named),*);
                    }
                }
                Fields::Unnamed(_) => panic!("Unnamed fields are not supported"),
                Fields::Unit => {
                    quote! {
                        pub async fn #handler_name();
                    }
                }
            }
        });

        quote! {
            #[async_trait]
            pub trait #handler_name {
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

    // TODO: either return the constructed packet enum from each arm, and pass it further
    // (like in the trait)
    // OR: make the trait not return anything,
    // and instead have each arm call the respective handler method.
    let parser = {
        let parser_arms = variants.clone().into_iter().map(|variant| {
            let variant_name = &variant.ident;

            match variant.fields {
                Fields::Unnamed(_) => {
                    panic!("Variants need to have named arguments")
                }
                Fields::Unit => {
                    quote! {
                        #name::#variant_name => (),
                    }
                }
                Fields::Named(named_args) => {
                    let named = named_args.clone().named.into_iter();
                    let deserialize_steps = named_args.named.into_iter().map(|field| {
                        match &field.ty {
                            Type::Path(type_path) => {
                                let field_ident =
                                    field.ident.clone().expect("Fields must have a name");
                                let type_name =
                                    type_path.path.get_ident().map(|ident| ident.to_string());

                                match type_name.as_deref() {
                                    Some("i8") | Some("i16") | Some("i32") | Some("i64")
                                    | Some("i128") | Some("isize") | Some("u8") | Some("u16")
                                    | Some("u32") | Some("u64") | Some("u128") | Some("usize")
                                    | Some("f32") | Some("f64") => {
                                        let primitive_name = type_path.path.get_ident().unwrap();
                                        quote! {
                                            let size = #primitive_name::BITS / 8;
                                            let #field_ident = #primitive_name::from_be_bytes(packet.payload[i..i+size].try_into().unwrap());
                                            i += size;
                                        }
                                    }
                                    Some("Uuid") => {
                                        quote! {
                                            let size = 16;
                                            let #field_ident = Uuid::from_slice(&packet.payload[i..i+size])?;
                                            i += size;
                                        }
                                    }
                                    _ => panic!("Unsupported datatype {:?}", &type_path.path),
                                }
                            }
                            _ => panic!("Bad type {:?}", &field.ty),
                        }
                    });

                    quote! {
                        #name::#variant_name { #(#named),* } => {
                            let i = 0usize;
                            #(#deserialize_steps)*
                            ()
                        }
                    }
                }
            }
        });

        quote! {
            struct #parser_name {};

            impl PacketParser<#name> for #parser_name {
                async fn parse_packet(
                    &self,
                    stream: &mut ReadHalf<TlsStream<TcpStream>>,
                ) -> Result<T, PacketParseError> {
                    let header = [0u8; 5];
                    stream.read_exact(&mut header)?;

                    let packet_type = #name::try_from(header[0])?;
                    let payload_length = u32::from_be_bytes(header[1..5].try_into().unwrap());
                    let mut payload = vec![0u8; header.payload_size as usize];
                    stream.read_exact(&mut payload)?;

                    match packet_type {
                        #(#parser_arms)*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #handler_type

        #u8_conversion

        #builder

        #parser
    };

    TokenStream::from(expanded)
}
