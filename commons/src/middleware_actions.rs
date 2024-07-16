pub fn remove_bearer_prefix(token: &str) -> String {
    if let Some(stripped) = token.strip_prefix("Bearer ") {
        stripped.to_string()
    } else {
        token.to_string()
    }
}
