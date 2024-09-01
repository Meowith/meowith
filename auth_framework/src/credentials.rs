use std::fmt::Debug;

pub trait AuthenticationCredentials: Send + Debug {
    fn get_authentication_identifier(&self) -> String;

    fn get_username(&self) -> Option<String>;

    fn is_setup(&self) -> bool {
        false
    }
}
