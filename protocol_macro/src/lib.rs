mod structs;
mod traits;
mod uses;
mod utils;

use crate::structs::dispatcher::generate_dispatcher_struct;
use crate::structs::serializer::generate_serializer_struct;
use crate::traits::from::generate_from;
use crate::traits::handler::generate_handler;
use crate::traits::validator::generate_validator;
use crate::uses::generate_uses;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Variant};

/// Derives the necessary traits and traits for a Meowith protocol
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

    let dispatcher_name = Ident::new(&format!("{}{}", name, "Dispatcher"), name.span());
    let serializer_name = Ident::new(&format!("{}{}", name, "Serializer"), name.span());
    let handler_name = Ident::new(&format!("{}{}", name, "Handler"), name.span());

    //traits
    let handler_type = generate_handler(handler_name.clone(), variants.clone());
    let from_traits = generate_from(name.clone(), variants.clone());
    let validator = generate_validator(name.clone(), variants.clone());
    let traits = quote! {
        #validator
        #handler_type
        #from_traits
    };

    //structs
    let dispatcher = generate_dispatcher_struct(
        name.clone(),
        variants.clone(),
        dispatcher_name,
        handler_name,
    );
    let serializer = generate_serializer_struct(name.clone(), variants.clone(), serializer_name);
    let structs = quote! {
        #serializer
        #dispatcher
    };

    let uses = generate_uses();

    let expanded = quote! {
        #uses
        #traits
        #structs
    };

    TokenStream::from(expanded)
}
