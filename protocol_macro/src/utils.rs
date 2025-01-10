use proc_macro2::Ident;
use syn::{Type, TypePath};

pub fn pascal_to_snake_case(ident: &Ident) -> String {
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

pub fn extract_type_name(path: &TypePath) -> String {
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
