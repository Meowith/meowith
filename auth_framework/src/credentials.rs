pub trait AuthenticationCredentials: Send {
    fn get_authentication_identifier(&self) -> String;

    fn get_username(&self) -> Option<String>;
}
