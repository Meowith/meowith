use rcgen::{generate_simple_self_signed};
use crate::error::certificate::CertificateError;

#[allow(unused)]
pub fn generate_certificate(domains: Vec<String>) -> Result<(String, String), CertificateError> {
    let cert = generate_simple_self_signed(domains).unwrap();

    let cert_pem = cert.cert.pem();
    let private_key_pem = cert.key_pair.serialize_pem();

    Ok((cert_pem, private_key_pem))
}