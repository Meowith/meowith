use auth_framework::token::DashboardClaims;
use data::dto::config::AccessTokenConfiguration;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[allow(unused)]
pub struct AuthenticationJwtService {
    pub(crate) validation: Validation,
    pub(crate) token_validity: u64,
    pub(crate) encoding_key: EncodingKey,
    pub(crate) decoding_key: DecodingKey,
    pub(crate) header: Header,
}

impl AuthenticationJwtService {
    pub fn generate_token(&self, claims: DashboardClaims) -> jsonwebtoken::errors::Result<String> {
        let now = SystemTime::now();
        let claims = Claims {
            sub: serde_json::to_string(&claims)?,
            exp: (now.duration_since(UNIX_EPOCH).unwrap().as_secs() + self.token_validity) as usize,
        };

        encode(&self.header, &claims, &self.encoding_key)
    }

    #[allow(unused)]
    pub fn verify_token(&self, token: &str) -> Result<DashboardClaims, Box<dyn std::error::Error>> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation);

        Ok(serde_json::from_str(&token_data?.claims.sub)?)
    }

    pub fn new(
        config: &AccessTokenConfiguration,
    ) -> Result<AuthenticationJwtService, Box<dyn std::error::Error>> {
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("JWT".to_string());

        Ok(AuthenticationJwtService {
            validation: Validation::new(Algorithm::HS256),
            token_validity: config.token_validity,
            encoding_key: EncodingKey::from_secret(config.secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(config.secret.as_bytes()),
            header,
        })
    }
}
