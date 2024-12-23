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

#[proc_macro_derive(Protocol)]
pub fn my_macro(input: TokenStream) -> TokenStream {
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

    // Derives:
    // Parser trait
    // Protocol trait

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
                            | Some("isize") => {
                                quote! { res.push(&#field_ident.to_be_bytes()); }
                            }
                            Some("u8") | Some("u16") | Some("u32") | Some("u64") | Some("u128")
                            | Some("usize") => {
                                quote! { res.push(&#field_ident.to_be_bytes()); }
                            }
                            Some("f32") | Some("f64") => {
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

                        #(#serialize_steps)*

                        res
                    }
                }
            }
        }
    });

    // TODO: akin to the mdsftp handler, it should pass it some sort of a channel and return some sort of Result<(), SomeError>
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

    let from_self_arms = variants.into_iter().map(|variant| {
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

    let expanded = quote! {
        #handler_type

        struct #parser_name {};

        struct #builder_name {};

        #u8_conversion

        impl PacketBuilder<#name> for #builder_name {
            fn build_packet<T>(&self, packet: T) -> Vec<u8> {
                match self {
                    #(#builder_arms)*
                }
            }
        }

        impl PacketParser<#name> for #parser_name {
            async fn parse_packet(
                &self,
                stream: &mut ReadHalf<TlsStream<TcpStream>>,
            ) -> Result<T, PacketParseError> {

            }
        }

    };

    TokenStream::from(expanded)
}
