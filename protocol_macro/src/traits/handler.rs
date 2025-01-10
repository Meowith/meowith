use crate::utils::pascal_to_snake_case;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Fields, Variant};

pub fn generate_handler(handler_name: Ident, variants: Vec<Variant>) -> TokenStream {
    let handler_methods = variants.into_iter().map(|variant| {
        let handler_method_name = format!("handle_{}", pascal_to_snake_case(&variant.ident));
        let handler_method_name = Ident::new(&handler_method_name, proc_macro2::Span::call_site());

        match variant.fields {
            Fields::Named(named_args) => {
                let named = named_args.named.into_iter();
                quote! {
                    async fn #handler_method_name(&self, writer: Arc<Mutex<PacketWriter<T>>>, #(#named),*) -> ProtocolResult<()>;
                }
            }
            Fields::Unnamed(_) => panic!("Unnamed fields are not supported"),
            Fields::Unit => {
                quote! {
                    async fn #handler_method_name(&self, writer: Arc<Mutex<PacketWriter<T>>>) -> ProtocolResult<()>;
                }
            }
        }
    });

    quote! {
        #[async_trait]
        pub trait #handler_name<T: Packet + 'static + Send>: Debug + Send + Sync {
            #(
                #handler_methods
            )*
        }
    }
}
