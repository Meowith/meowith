extern crate openssl;

use std::error::Error;
use openssl::error::ErrorStack;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::x509::extension::SubjectAlternativeName;
use openssl::x509::{X509NameBuilder, X509Req, X509ReqBuilder, X509};
use std::net::IpAddr;
use crate::context::{NodeRequestContext, RequestContext};

pub struct SigningData {
    pub ip_addr: IpAddr,
    pub validity_days: u32,
}

pub fn generate_private_key(bits: u32) -> Result<PKey<Private>, ErrorStack> {
    let rsa = Rsa::generate(bits)?;
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
    cert_builder.sign(ca_private_key, openssl::hash::MessageDigest::sha256())?;

    let not_before = openssl::asn1::Asn1Time::days_from_now(0)?;
    let not_after = openssl::asn1::Asn1Time::days_from_now(req_data.validity_days)?;
    cert_builder.set_not_before(&not_before)?;
    cert_builder.set_not_after(&not_after)?;

    let san_extension = SubjectAlternativeName::new()
        .ip(&req_data.ip_addr.to_string())
        .build(&cert_builder.x509v3_context(None, None))?;
    cert_builder.append_extension(san_extension)?;

    Ok(cert_builder.build())
}

pub async fn perform_certificate_request(ctx: &NodeRequestContext) -> Result<(PKey<Private>, X509), Box<dyn Error>> {
    let pkey = generate_private_key(4096)?;
    let csr = generate_csr(&pkey)?;

    let resp = ctx.client()
        .post(ctx.controller("/api/internal/security/csr"))
        .body(csr.to_der()?)
        .send().await?
        .bytes().await?;

    Ok((pkey, X509::from_der(resp.as_ref())?))
}

#[cfg(test)]
mod tests {
    use crate::autoconfigure::ssl_conf::{
        generate_csr, generate_private_key, sign_csr, SigningData,
    };
    use openssl::hash::MessageDigest;
    use openssl::nid::Nid;
    use openssl::pkey::{PKey, Private};
    use openssl::rsa::Rsa;
    use openssl::x509::extension::BasicConstraints;
    use openssl::x509::X509VerifyResult;
    use openssl::x509::{X509NameBuilder, X509};
    use std::net::IpAddr;
    use std::str::FromStr;

    fn gen_ca() -> (X509, PKey<Private>) {
        let rsa = Rsa::generate(2048).unwrap();
        let pkey = PKey::from_rsa(rsa).unwrap();

        let mut x509_name = X509NameBuilder::new().unwrap();
        x509_name
            .append_entry_by_nid(Nid::COMMONNAME, "example.com")
            .unwrap();
        let x509_name = x509_name.build();

        let mut cert_builder = X509::builder().unwrap();
        cert_builder.set_version(2).unwrap();
        cert_builder.set_subject_name(&x509_name).unwrap();
        cert_builder.set_issuer_name(&x509_name).unwrap();
        cert_builder.set_pubkey(&pkey).unwrap();

        let basic_constraints = BasicConstraints::new().critical().ca().build().unwrap();
        cert_builder.append_extension(basic_constraints).unwrap();

        cert_builder.sign(&pkey, MessageDigest::sha256()).unwrap();

        (cert_builder.build(), pkey)
    }

    #[test]
    fn test_request() {
        let (ca, ca_private_key) = gen_ca();

        let key = generate_private_key(2048).expect("Key gen failed");
        let request = generate_csr(&key).expect("CSR gen failed");

        let cert = sign_csr(
            &request,
            &ca,
            &ca_private_key,
            &SigningData {
                ip_addr: IpAddr::from_str("1.2.3.4").unwrap(),
                validity_days: 100,
            },
        )
        .expect("CRS sign failed");

        let success = match ca.issued(&cert) {
            X509VerifyResult::OK => true,
            _ => false,
        };

        assert!(success);
    }
}
