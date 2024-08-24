extern crate openssl;

use crate::context::microservice_request_context::MicroserviceRequestContext;
use crate::context::request_context::RequestContext;
use data::dto::controller::X_ADDR_HEADER;
use openssl::bn::{BigNum, MsbOption};
use openssl::error::ErrorStack;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::x509::extension::{
    AuthorityKeyIdentifier, KeyUsage, SubjectAlternativeName, SubjectKeyIdentifier,
};
use openssl::x509::{X509NameBuilder, X509Req, X509ReqBuilder, X509};
use std::error::Error;
use std::net::IpAddr;

pub struct SigningData {
    pub ip_addr: IpAddr,
    pub validity_days: u32,
}

pub fn generate_private_key() -> Result<PKey<Private>, ErrorStack> {
    let rsa = Rsa::generate(4096)?;
    PKey::from_rsa(rsa)
}

pub fn generate_csr(private_key: &PKey<Private>) -> Result<X509Req, ErrorStack> {
    let mut name_builder = X509NameBuilder::new()?;
    name_builder.append_entry_by_text("C", "US")?;
    let name = name_builder.build();

    let mut req_builder = X509ReqBuilder::new()?;
    req_builder.set_pubkey(private_key)?;
    req_builder.set_subject_name(&name)?;
    req_builder.sign(private_key, openssl::hash::MessageDigest::sha256())?;
    Ok(req_builder.build())
}

pub fn sign_csr(
    csr: &X509Req,
    ca_cert: &X509,
    ca_private_key: &PKey<Private>,
    req_data: &SigningData,
) -> Result<X509, ErrorStack> {
    let mut cert_builder = X509::builder()?;
    cert_builder.set_version(2)?;
    cert_builder.set_subject_name(csr.subject_name())?;
    cert_builder.set_issuer_name(ca_cert.issuer_name())?;
    cert_builder.set_pubkey(&csr.public_key().unwrap())?;
    let serial_number = {
        let mut serial = BigNum::new()?;
        serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
        serial.to_asn1_integer()?
    };
    cert_builder.set_serial_number(&serial_number)?;

    let not_before = openssl::asn1::Asn1Time::days_from_now(0)?;
    let not_after = openssl::asn1::Asn1Time::days_from_now(req_data.validity_days)?;
    cert_builder.set_not_before(&not_before)?;
    cert_builder.set_not_after(&not_after)?;

    let san_extension = SubjectAlternativeName::new()
        .ip(&req_data.ip_addr.to_string())
        .build(&cert_builder.x509v3_context(None, None))?;
    cert_builder.append_extension(san_extension)?;

    cert_builder.append_extension(
        KeyUsage::new()
            .critical()
            .non_repudiation()
            .digital_signature()
            .key_encipherment()
            .build()?,
    )?;

    let subject_key_identifier =
        SubjectKeyIdentifier::new().build(&cert_builder.x509v3_context(Some(ca_cert), None))?;
    cert_builder.append_extension(subject_key_identifier)?;

    let auth_key_identifier = AuthorityKeyIdentifier::new()
        .keyid(false)
        .issuer(false)
        .build(&cert_builder.x509v3_context(Some(ca_cert), None))?;
    cert_builder.append_extension(auth_key_identifier)?;

    cert_builder.sign(ca_private_key, openssl::hash::MessageDigest::sha256())?;
    Ok(cert_builder.build())
}

/// Expects BOTH of the tokens.
pub async fn perform_certificate_request(
    ctx: &MicroserviceRequestContext,
    addr: IpAddr,
) -> Result<(PKey<Private>, X509), Box<dyn Error>> {
    let pkey = generate_private_key()?;
    let csr = generate_csr(&pkey)?;

    let resp = ctx
        .client()
        .await
        .post(ctx.controller("/api/internal/security/csr"))
        .header(
            "Sec-Authorization",
            ctx.security_context.renewal_token.clone(),
        )
        .header(X_ADDR_HEADER, addr.to_string())
        .body(csr.to_der()?)
        .send()
        .await?
        .bytes()
        .await?;

    Ok((pkey, X509::from_der(resp.as_ref())?))
}

/// Used for generating ssl certs for tests.
pub fn gen_test_ca() -> (X509, PKey<Private>) {
    use openssl::asn1::Asn1Time;
    use openssl::bn::{BigNum, MsbOption};
    use openssl::hash::MessageDigest;
    use openssl::nid::Nid;
    use openssl::x509::extension::BasicConstraints;
    use openssl::x509::extension::KeyUsage;

    let rsa = Rsa::generate(2048).unwrap();
    let pkey = PKey::from_rsa(rsa).unwrap();

    let mut x509_name = X509NameBuilder::new().unwrap();
    x509_name
        .append_entry_by_nid(Nid::COMMONNAME, "127.0.0.1")
        .unwrap();
    let x509_name = x509_name.build();

    let mut cert_builder = X509::builder().unwrap();
    cert_builder.set_version(2).unwrap();
    let serial_number = {
        let mut serial = BigNum::new().unwrap();
        serial.rand(159, MsbOption::MAYBE_ZERO, false).unwrap();
        serial.to_asn1_integer().unwrap()
    };
    cert_builder.set_serial_number(&serial_number).unwrap();
    cert_builder.set_subject_name(&x509_name).unwrap();
    cert_builder.set_issuer_name(&x509_name).unwrap();
    cert_builder.set_pubkey(&pkey).unwrap();
    let not_before = Asn1Time::days_from_now(0).unwrap();
    cert_builder.set_not_before(&not_before).unwrap();
    let not_after = Asn1Time::days_from_now(365).unwrap();
    cert_builder.set_not_after(&not_after).unwrap();

    let basic_constraints = BasicConstraints::new().critical().ca().build().unwrap();
    cert_builder.append_extension(basic_constraints).unwrap();
    cert_builder
        .append_extension(
            KeyUsage::new()
                .critical()
                .key_cert_sign()
                .crl_sign()
                .build()
                .unwrap(),
        )
        .unwrap();

    let subject_key_identifier = openssl::x509::extension::SubjectKeyIdentifier::new()
        .build(&cert_builder.x509v3_context(None, None))
        .unwrap();
    cert_builder
        .append_extension(subject_key_identifier)
        .unwrap();

    cert_builder.sign(&pkey, MessageDigest::sha256()).unwrap();

    (cert_builder.build(), pkey)
}

pub fn gen_test_certs(ca: &X509, ca_key: &PKey<Private>) -> (X509, PKey<Private>) {
    use std::str::FromStr;

    let key = generate_private_key().expect("Key gen failed");
    let request = generate_csr(&key).expect("CSR gen failed");

    let cert = sign_csr(
        &request,
        ca,
        ca_key,
        &SigningData {
            ip_addr: IpAddr::from_str("127.0.0.1").unwrap(),
            validity_days: 100,
        },
    )
    .expect("CRS sign failed");

    (cert, key)
}

#[cfg(test)]
pub mod tests {
    use openssl::x509::X509VerifyResult;

    use crate::autoconfigure::ssl_conf::{gen_test_ca, gen_test_certs};

    #[test]
    fn test_request() {
        let (ca, ca_private_key) = gen_test_ca();
        let (cert, _key) = gen_test_certs(&ca, &ca_private_key);

        let success = match ca.issued(&cert) {
            X509VerifyResult::OK => true,
            _ => false,
        };

        assert!(success);
    }
}
