use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields};

#[proc_macro_derive(Protocol)]
pub fn my_macro(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = if let Data::Enum(data_enum) = &input.data {
        data_enum.variants.iter().filter_map(|variant| {
            if let Fields::Named(_) = &variant.fields {
                Some(variant)
            } else {
                None
            }
        })
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
    // Builder trait
    // Parser trait
    // Protocol trait
    // Handler type

    let builder_arms = variants.clone().map(|variant| {
        let variant_name = &variant.ident;

        match &variant.fields {
            Fields::Unnamed(_) => {
                panic!("Variants need to have named arguments")
            }
            Fields::Unit => {
                quote! {
                    #name::#variant_name => Vec::new(),
                }
            }
            Fields::Named(_) => {
                quote! {
                    #name::#variant_name { .. } => Vec::new(), // Handle named fields as needed
                }
            }
        }
    });

    let mut i = 0;
    let try_from_u8_arms = variants.clone().map(|variant| {
        let variant_name = &variant.ident;

        i += 1;
        quote! {
            #i => #name::#variant_name
        }
    });

    let mut i = 0;

    let from_self_arms = variants.map(|variant| {
        let variant_name = &variant.ident;

        i += 1;
        quote! {
            #name::#variant_name => #i
        }
    });

    let u8_conversion = quote! {
        impl TryFrom<u8> for #name {
            type Error = ();

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    #(#try_from_u8_arms)*
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
        struct #parser_name {};

        #u8_conversion
        impl PacketBuilder<#name> for #parser_name {
            fn build_packet<T>(&self, packet: T) -> Vec<u8> {
                match self {
                    #(#builder_arms)*
                }
            }
        }

        struct #builder_name {};
    };

    TokenStream::from(expanded)
}