use std::net::IpAddr;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rcgen::{Certificate, CertificateParams, CertificateSigningRequest, KeyPair, SanType};
use rsa::pkcs1::{EncodeRsaPrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use time::OffsetDateTime;

pub fn generate_private_key(bits: usize) -> RsaPrivateKey {
    let mut rng = rand::thread_rng();

    RsaPrivateKey::new(&mut rng, bits).expect("Failed to generate the private key")
}

pub fn generate_signing_request(
    address: String,
    validity_days: u16,
    key: RsaPrivateKey,
) -> CertificateSigningRequest {
    let mut params = CertificateParams::default();
    params
        .subject_alt_names
        .push(match IpAddr::from_str(&address.as_str()) {
            Ok(ip) => SanType::IpAddress(ip),
            Err(_) => SanType::DnsName(address.try_into()?),
        });

    let now = SystemTime::now();
    params.not_before = OffsetDateTime::from_unix_timestamp(
        now.duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64,
    )
    .unwrap();
    params.not_after = OffsetDateTime::from_unix_timestamp(
        (now + Duration::from_days(validity_days as u64))
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64,
    )
    .unwrap();

    params
        .serialize_request(KeyPair::from_pem(
            key.to_pkcs1_pem(LineEnding::CRLF).unwrap(),
        ))
        .unwrap()
}

pub fn request_certificate() {}
