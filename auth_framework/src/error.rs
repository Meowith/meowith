#[derive(Debug)]
pub enum AuthenticateError {
    InvalidCredentials,
    InternalError,
}

#[derive(Debug)]
pub enum AuthCredentialsError {
    InvalidCredentialsFormat,
    InternalError,
}
