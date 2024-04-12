use std::collections::HashSet;
use std::mem;
use actix::{Message, Handler, StreamHandler, Actor, SpawnHandle, Addr};
use actix_web::{HttpRequest, HttpResponse};
use actix_web::web;
use actix_web_actors::ws;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{DecodingKey, Validation, Algorithm};
use alerion_config::AlerionConfig;
use bytestring::ByteString;

macro_rules! extract_args {
    ($json_struct:ident, $closure:expr) => {
        {
            let ::std::option::Option::Some(inner) = $json_struct.get("args")
                .and_then(|a| a.get(0))
                .and_then($closure) else {
                return;
            };

            inner
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
enum EventType {
    #[serde(rename = "auth")]
    Authentication,
    #[serde(rename = "auth success")]
    AuthenticationSuccess,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "install completed")]
    InstallCompleted,
}

#[derive(Debug, Default)]
struct Permissions {
    pub connect: bool,
}

impl Permissions {
    pub fn from_strings(strings: &[impl AsRef<str>]) -> Self {
        let mut this = Permissions::default();

        for s in strings {
            match s.as_ref() {
                "*" => {
                    this.connect = true;
                }
                "websocket.connect" => { this.connect = true; }
                _what => {
                    // unknown permission..
                }
            }
        }

        this
    }
}

#[derive(Debug, Default)]
enum AuthState {
    #[default]
    Unauthenticated,
    WithPermissions(Permissions),
}

#[derive(Debug, Serialize)]
struct OutgoingEvent {
    event: EventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

impl OutgoingEvent {
    pub fn new_no_args(event: EventType) -> Self {
        OutgoingEvent {
            event,
            args: None,
        }
    }
}

impl From<OutgoingEvent> for ByteString {
    fn from(value: OutgoingEvent) -> Self {
        // there is no way this could fail
        serde_json::to_string(&value).unwrap().into()
    }
}

pub struct WebsocketConnection {
    uuid: Uuid,
    auth_state: AuthState,
    jwt_validation: Validation,
    jwt_key: DecodingKey,
    handle: Option<SpawnHandle>,
}

impl WebsocketConnection {
    pub fn new(uuid: Uuid, config: &AlerionConfig) -> Self {
        let mut jwt_validation = Validation::new(Algorithm::HS256);
        jwt_validation.required_spec_claims = HashSet::from(["exp", "nbf", "aud", "iss"].map(ToOwned::to_owned));
        jwt_validation.leeway = 10;
        jwt_validation.reject_tokens_expiring_in_less_than = 0;
        // todo: check if jsonwebtoken does exp/nbf validation properly
        jwt_validation.validate_exp = false;
        jwt_validation.validate_nbf = false;
        // skip audience validation, not like it's much useful anyways
        jwt_validation.validate_aud = false;
        jwt_validation.aud = None;
        jwt_validation.iss = Some(HashSet::from([config.remote.clone()]));
        jwt_validation.sub = None;
        

        WebsocketConnection {
            uuid,
            auth_state: AuthState::default(),
            jwt_validation,
            jwt_key: DecodingKey::from_secret(config.token.as_ref()),
            handle: None,
        }
    }

    pub fn start(
        self,
        req: &HttpRequest,
        payload: web::Payload,
    ) -> actix_web::Result<(Addr<WebsocketConnection>, HttpResponse)> {
        ws::WsResponseBuilder::new(self, req, payload).start_with_addr()
    }

    pub(crate) fn claims_valid(&self, claims: &JwtClaims) -> bool {
        claims.server_uuid == self.uuid
    }
}

impl Actor for WebsocketConnection {
    type Context = ws::WebsocketContext<Self>;
}

pub struct Msg;

impl Message for Msg {
    type Result = ();
}

impl Handler<Msg> for WebsocketConnection {
    type Result = ();

    fn handle(&mut self, _msg: Msg, ctx: &mut Self::Context) -> Self::Result {
        println!("?????");
        ctx.text(ByteString::from("{\"event\": \"logs\"}"));
        ()
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebsocketConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message;
        // just ignore bad messages
        // messages should always be Text
        let Ok(Message::Text(msg)) = msg else { return; };

        // todo: behavior on bad JSON payload? right now just ignore

        let Ok(json_struct) = serde_json::from_str::<serde_json::Value>(&msg) else {
            return;
        };

        let Some(event): Option<EventType> = json_struct.get("event")
            .and_then(|v| {
                if v.is_string() {
                    serde_json::from_value(v.clone()).ok()
                } else {
                    None
                }
            })
        else {
            return;
        };

        match event {
            EventType::Authentication => {
                let jwt = extract_args!(json_struct, |value| value.as_str());
                let result = jsonwebtoken::decode::<JwtClaims>(jwt, &self.jwt_key, &self.jwt_validation);

                if let Ok(mut token_data) = result {
                    if self.claims_valid(&token_data.claims) {
                        let permission_strings = mem::take(&mut token_data.claims.permissions);
                        let permissions = Permissions::from_strings(&permission_strings);

                        if permissions.connect {
                            self.auth_state = AuthState::WithPermissions(permissions);

                            ctx.text(OutgoingEvent::new_no_args(EventType::AuthenticationSuccess));
                        }
                    }
                }
            }

            _ => {
                if let AuthState::WithPermissions(ref _perms) = self.auth_state {
                    match event {
                        _ => {}
                    }
                }
            }
        }
    }
}

