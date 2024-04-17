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

#[derive(Debug, Default)]
pub struct Permissions {
    pub connect: bool,
    pub start: bool,
    pub stop: bool,
    pub restart: bool,
    pub console: bool,
    pub backup_read: bool,
    pub admin_errors: bool,
    pub admin_install: bool,
    pub admin_transfer: bool,
}

impl Permissions {
    pub fn from_strings(strings: &[impl AsRef<str>]) -> Self {
        let mut this = Permissions::default();

        for s in strings {
            match s.as_ref() {
                "*" => {
                    this.connect = true;
                    this.start = true;
                    this.stop = true;
                    this.restart = true;
                    this.console = true;
                    this.backup_read = true;
                }
                "websocket.connect" => {
                    this.connect = true;
                }
                "control.start" => {
                    this.start = true;
                }
                "control.stop" => {
                    this.stop = true;
                }
                "control.restart" => {
                    this.restart = true;
                }
                "control.console" => {
                    this.console = true;
                }
                "backup.read" => {
                    this.backup_read = true;
                }
                "admin.websocket.errors" => {
                    this.admin_errors = true;
                }
                "admin.websocket.install" => {
                    this.admin_install = true;
                }
                "admin.websocket.transfer" => {
                    this.admin_transfer = true;
                }
                _ => {}
            }
        }

        this
    }
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

        let key = DecodingKey::from_secret(cfg.auth.token.as_ref());

        Self { validation, key }
    }

    pub fn is_valid(&self, auth: &str, server_uuid: &Uuid) -> bool {
        jsonwebtoken::decode::<Claims>(auth, &self.key, &self.validation)
            .ok()
            .filter(|result| &result.claims.server_uuid == server_uuid)
            .is_some()
    }
}
