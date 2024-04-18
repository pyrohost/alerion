use std::convert::Infallible;

use actix::{Actor, ActorContext, Addr, Handler, StreamHandler};
use actix_web_actors::ws;
use alerion_datamodel::websocket::*;
use uuid::Uuid;

use crate::config::AlerionConfig;

use super::auth::Auth;
use super::relay::ServerConnection;

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
    ReceiveStats,
}

impl actix::Message for PanelMessage {
    type Result = Result<(), Infallible>;
}

pub type ConnectionAddr = Addr<WebsocketConnectionImpl>;

pub struct WebsocketConnectionImpl {
    server_uuid: Uuid,
    server_conn: ServerConnection,
    auth: Auth,
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
                let str = serde_json::to_string(&stats).expect("JSON serialization should not fail");
                ctx.text(RawMessage::new(EventType::Stats, str))
            }
        }

        Ok(())
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebsocketConnectionImpl {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message;
        // just ignore bad messages
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
        }
    }

    pub fn handle_text(&self, msg: &str, ctx: &mut <Self as Actor>::Context) -> Option<()> {
        // todo: behavior on bad JSON payload? right now just ignore
        let event = serde_json::from_str::<RawMessage>(msg).ok()?;

        match event.event() {
            EventType::Authentication => {
                let maybe_permissions = self.auth.validate(&event.into_first_arg()?, &self.server_uuid);

                if let Some(permissions) = maybe_permissions {
                    self.server_conn.set_authenticated();
                    ctx.text(RawMessage::new_no_args(EventType::AuthenticationSuccess));
                }

                Some(())
            }

            ty => {
                if self.server_conn.is_authenticated() {
                    match ty {
                        EventType::SendCommand => {
                            self.server_conn.send_if_authenticated(|| {
                                PanelMessage::Command("silly".to_owned())
                            });
                        }

                        EventType::SendStats => {
                            self.server_conn
                                .send_if_authenticated(|| PanelMessage::ReceiveStats);
                        }

                        EventType::SendLogs => {
                            self.server_conn
                                .send_if_authenticated(|| PanelMessage::ReceiveLogs);
                        }

                        e => todo!("{e:?}"),
                    }
                }

                Some(())
            }
        }
    }
}
