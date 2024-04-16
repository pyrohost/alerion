use std::collections::HashSet;

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AlerionConfig;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    aud: Vec<String>,
    jti: String,
    iat: usize,
    nbf: usize,
    exp: usize,
    server_uuid: Uuid,
    permissions: Vec<String>,
    user_uuid: Uuid,
    user_id: usize,
    unique_id: String,
}

pub struct Auth {
    validation: Validation,
    key: DecodingKey,
}

impl Auth {
    pub fn from_config(cfg: &AlerionConfig) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.required_spec_claims =
            HashSet::from(["exp", "nbf", "aud", "iss"].map(ToOwned::to_owned));
        validation.leeway = 10;
        validation.reject_tokens_expiring_in_less_than = 0;
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        validation.aud = None;
        validation.iss = Some(HashSet::from([cfg.remote.clone()]));
        validation.sub = None;

        let key = DecodingKey::from_secret(cfg.token.as_ref());

        Self { validation, key }
    }

    pub fn is_valid(&self, auth: &str, server_uuid: &Uuid) -> bool {
        jsonwebtoken::decode::<Claims>(auth, &self.key, &self.validation)
            .ok()
            .filter(|result| &result.claims.server_uuid == server_uuid)
            .is_some()
    }
}
