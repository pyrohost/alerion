use std::collections::HashSet;

use bitflags::bitflags;
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

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct Permissions: u32 {
        const CONNECT = 1;
        const START = 1 << 1;
        const STOP = 1 << 2;
        const RESTART = 1 << 3;
        const CONSOLE = 1 << 4;
        const BACKUP_READ = 1 << 5;
        const ADMIN_ERRORS = 1 << 6;
        const ADMIN_INSTALL = 1 << 7;
        const ADMIN_TRANSFER = 1 << 8;
    }
}

impl Permissions {
    pub fn from_strings(strings: &[impl AsRef<str>]) -> Self {
        let mut this = Permissions::empty();

        for s in strings {
            match s.as_ref() {
                "*" => {
                    this.insert(Permissions::CONNECT);
                    this.insert(Permissions::START);
                    this.insert(Permissions::STOP);
                    this.insert(Permissions::RESTART);
                    this.insert(Permissions::CONSOLE);
                    this.insert(Permissions::BACKUP_READ);
                    
                }
                "websocket.connect" => {
                    this.insert(Permissions::CONNECT);
                }
                "control.start" => {
                    this.insert(Permissions::START);
                }
                "control.stop" => {
                    this.insert(Permissions::STOP);
                }
                "control.restart" => {
                    this.insert(Permissions::RESTART);
                }
                "control.console" => {
                    this.insert(Permissions::CONSOLE);
                }
                "backup.read" => {
                    this.insert(Permissions::BACKUP_READ);
                }
                "admin.websocket.errors" => {
                    this.insert(Permissions::ADMIN_ERRORS);
                }
                "admin.websocket.install" => {
                    this.insert(Permissions::ADMIN_INSTALL);
                }
                "admin.websocket.transfer" => {
                    this.insert(Permissions::ADMIN_TRANSFER);
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

        let spec_claims = ["exp", "nbf", "aud", "iss"].map(ToOwned::to_owned);

        validation.required_spec_claims = HashSet::from(spec_claims);
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

    pub fn validate(&self, auth: &str, server_uuid: &Uuid) -> Option<Permissions> {
        jsonwebtoken::decode::<Claims>(auth, &self.key, &self.validation)
            .ok()
            .filter(|result| &result.claims.server_uuid == server_uuid)
            .map(|result| Permissions::from_strings(&result.claims.permissions))
    }
}
