use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use data::dto::config::AccessTokenConfiguration;
use data::model::app_model::App;
use data::model::user_model::User;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Permit {
    pub scope: String,
    pub allowance: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimData {
    pub app_id: Uuid,
    pub issuer_id: Uuid,
    pub name: String,
    pub nonce: Uuid,
    pub perms: Vec<Permit>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ClaimKey {
    pub app_id: Uuid,
    pub issuer_id: Uuid,
    pub name: String,
    pub nonce: Uuid,
}

impl From<&ClaimData> for ClaimKey {
    fn from(value: &ClaimData) -> Self {
        ClaimKey {
            app_id: value.app_id,
            issuer_id: value.issuer_id,
            name: value.name.clone(),
            nonce: value.nonce,
        }
    }
}

pub struct JwtService {
    pub(crate) validation: Validation,
    pub(crate) token_validity: u64,
    pub(crate) encoding_key: EncodingKey,
    pub(crate) decoding_key: DecodingKey,
    pub(crate) header: Header,
}

impl JwtService {
    pub fn generate_token(
        &self,
        issuer: &User,
        app: &App,
        token_name: &str,
        perms: Vec<Permit>,
        nonce: Uuid,
    ) -> jsonwebtoken::errors::Result<String> {
        let now = SystemTime::now();
        let claims = Claims {
            sub: serde_json::to_string(&ClaimData {
                app_id: app.id,
                issuer_id: issuer.id,
                name: token_name.to_owned(),
                nonce,
                perms,
            })?,
            exp: (now.duration_since(UNIX_EPOCH).unwrap().as_secs() + self.token_validity) as usize,
        };

        encode(&self.header, &claims, &self.encoding_key)
    }

    pub fn verify_token(&self, token: &str) -> Result<ClaimData, Box<dyn Error>> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation);

        Ok(serde_json::from_str(&token_data?.claims.sub)?)
    }

    pub fn new(config: &AccessTokenConfiguration) -> Result<JwtService, Box<dyn Error>> {
        let mut header = Header::new(Algorithm::HS384);
        header.typ = Some("JWT".to_string());

        Ok(JwtService {
            validation: Validation::new(Algorithm::HS384),
            token_validity: config.token_validity,
            encoding_key: EncodingKey::from_base64_secret(config.secret.as_str())?,
            decoding_key: DecodingKey::from_base64_secret(config.secret.as_str())?,
            header,
        })
    }
}
