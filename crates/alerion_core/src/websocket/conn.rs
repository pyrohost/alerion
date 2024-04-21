use std::cell::Cell;
use std::convert::Infallible;

use actix::{Actor, ActorContext, Addr, Handler, StreamHandler};
use actix_web_actors::ws;
use alerion_datamodel::websocket::*;
use uuid::Uuid;

use crate::config::AlerionConfig;
use crate::websocket::auth::{Auth, Permissions};
use crate::websocket::relay::ServerConnection;

#[derive(Debug, Clone)]
pub enum ServerMessage {
    Kill,
    Logs(String),
    Stats(PerformanceStatisics),
}

impl actix::Message for ServerMessage {
    type Result = Result<(), Infallible>;
}

#[derive(Debug, Clone)]
pub enum PanelMessage {
    Command(String),
    ReceiveLogs,
    ReceiveInstallLog,
    ReceiveStats,
}

impl actix::Message for PanelMessage {
    type Result = Result<(), Infallible>;
}

pub type ConnectionAddr = Addr<WebsocketConnectionImpl>;

//TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
enum MessageError {
    InvalidJwt,
    Generic(String),
}

pub struct WebsocketConnectionImpl {
    server_uuid: Uuid,
    server_conn: ServerConnection,
    auth: Auth,
    permissions: Cell<Permissions>,
}

impl Actor for WebsocketConnectionImpl {
    type Context = ws::WebsocketContext<Self>;
}

impl Handler<ServerMessage> for WebsocketConnectionImpl {
    type Result = Result<(), Infallible>;

    fn handle(&mut self, msg: ServerMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ServerMessage::Kill => {
                ctx.close(None);
                ctx.stop();
            }

            ServerMessage::Logs(logs) => {
                ctx.text(RawMessage::new(EventType::Logs, logs));
            }

            ServerMessage::Stats(stats) => {
                let str =
                    serde_json::to_string(&stats).expect("JSON serialization should not fail");
                ctx.text(RawMessage::new(EventType::Stats, str))
            }
        }

        Ok(())
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebsocketConnectionImpl {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message;

        let Ok(msg) = item else {
            return;
        };

        match msg {
            Message::Text(t) => {
                let _result = self.handle_text(&t, ctx);
            }
            _ => println!("TODO: non-text WS msgs"),
        }
    }
}

impl WebsocketConnectionImpl {
    pub fn new(server_uuid: Uuid, server_conn: ServerConnection, cfg: &AlerionConfig) -> Self {
        Self {
            server_uuid,
            server_conn,
            auth: Auth::from_config(cfg),
            permissions: Cell::new(Permissions::empty()),
        }
    }

    pub fn id(&self) -> Uuid {
        self.server_uuid
    }

    pub fn handle_text(&self, msg: &str, ctx: &mut <Self as Actor>::Context) -> Option<()> {
        // todo: behavior on bad JSON payload? right now just ignore
        let event = serde_json::from_str::<RawMessage>(msg).ok()?;

        match event.event() {
            EventType::Authentication => {
                let maybe_permissions = self
                    .auth
                    .validate(&event.into_first_arg()?, &self.server_uuid);

                if let Some(permissions) = maybe_permissions {
                    if permissions.contains(Permissions::CONNECT) {
                        self.permissions.set(permissions);
                        self.server_conn.set_authenticated();
                        ctx.text(RawMessage::new_no_args(EventType::AuthenticationSuccess));
                    }
                } else {
                    self.send_error(ctx, MessageError::InvalidJwt);
                }

                Some(())
            }

            ty => {
                if self.server_conn.is_authenticated() {
                    let permissions = self.permissions.get();

                    match ty {
                        EventType::SendCommand => {
                            if permissions.contains(Permissions::CONSOLE) {
                                if let Some(command) = event.into_first_arg() {
                                    self.server_conn
                                        .send_if_authenticated(PanelMessage::Command(command));
                                } else {
                                    self.send_error(ctx, MessageError::InvalidJwt);
                                }
                            }
                        }

                        EventType::SendStats => {
                            if permissions.contains(Permissions::CONSOLE) {
                                self.server_conn
                                    .send_if_authenticated(PanelMessage::ReceiveStats);
                            }
                        }

                        EventType::SendLogs => {
                            if permissions.contains(Permissions::CONSOLE) {
                                self.server_conn
                                    .send_if_authenticated(PanelMessage::ReceiveLogs);

                                if permissions.contains(Permissions::ADMIN_INSTALL) {
                                    self.server_conn.force_send(PanelMessage::ReceiveInstallLog);
                                }
                            }
                        }

                        e => todo!("{e:?}"),
                    }
                }

                Some(())
            }
        }
    }

    #[inline(always)]
    fn send_error(&self, ctx: &mut <Self as Actor>::Context, err: MessageError) {
        let precise_errors = self.permissions.get().contains(Permissions::ADMIN_ERRORS);

        let raw_msg = if precise_errors {
            match err {
                MessageError::InvalidJwt => RawMessage::new_no_args(EventType::JwtError),
                MessageError::Generic(s) => RawMessage::new(EventType::DaemonError, s),
            }
        } else {
            RawMessage::new(
                EventType::DaemonError,
                "An unexpected error occurred".to_owned(),
            )
        };

        ctx.text(raw_msg)
    }
}
