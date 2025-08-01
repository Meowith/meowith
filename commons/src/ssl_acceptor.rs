use openssl::pkey::{PKey, Private};
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
use openssl::x509::X509;
use std::path::Path;

pub fn build_provided_ssl_acceptor_builder(
    private_key_path: &Path,
    public_key_path: &Path,
) -> SslAcceptorBuilder {
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file(private_key_path, SslFiletype::PEM)
        .unwrap_or_else(|_| panic!("Private key {private_key_path:?} could not be accessed"));
    builder
        .set_certificate_chain_file(public_key_path)
        .unwrap_or_else(|_| {
            panic!("Ssl certificate key {public_key_path:?} could not be accessed")
        });

    builder
}

pub fn build_autogen_ssl_acceptor_builder(
    certificate: X509,
    private_key: PKey<Private>,
) -> SslAcceptorBuilder {
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key(&private_key)
        .expect("An mdsftp_error occurred during auto gen ssl acceptor init");
    builder
        .set_certificate(&certificate)
        .expect("An mdsftp_error occurred during auto gen ssl acceptor init");
    builder
}
