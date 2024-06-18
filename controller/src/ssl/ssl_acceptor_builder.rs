use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
use crate::config::controller_config::ControllerConfig;

pub fn build_ssl_acceptor_builder(config: ControllerConfig, use_ssl: &mut bool) -> SslAcceptorBuilder {
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    if config.ssl_certificate.is_some() && config.ssl_private_key.is_some() {
        builder
            .set_private_key_file(config.ssl_private_key.clone().unwrap(), SslFiletype::PEM)
            .unwrap_or_else(|_| {
                panic!(
                    "Private key {} could not be accessed",
                    &config.ssl_private_key.unwrap()
                )
            });
        builder
            .set_certificate_chain_file(config.ssl_certificate.clone().unwrap())
            .unwrap_or_else(|_| {
                panic!(
                    "Ssl certificate key {} could not be accessed",
                    &config.ssl_certificate.unwrap()
                )
            });
        *use_ssl = true;
    }

    builder
}