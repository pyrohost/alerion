use std::convert::Infallible;
use actix_web_actors::ws;
use actix::{Handler, StreamHandler, Actor, Addr, ActorContext};
use uuid::Uuid;
use crate::config::AlerionConfig;
use crate::websocket::message::PanelMessage;
use super::message::ServerMessage;
use super::serde_driver::{EventType, IncomingEvent, OutgoingEvent};
use super::auth::Auth;
use super::relay::ServerConnection;

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
        }

        Ok(())
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebsocketConnectionImpl {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) { 
        use ws::Message;
        // just ignore bad messages
        let Ok(msg) = item else { return; };

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
        let event = IncomingEvent::try_parse(msg)?;

        match event.event() {
            EventType::Authentication => {
                if self.auth.is_valid(&event.into_first_arg_as_str()?, &self.server_uuid) {
                    self.server_conn.set_authenticated();
                    ctx.text(OutgoingEvent::new_no_args(EventType::AuthenticationSuccess));
                }

                Some(())
            }

            ty => {
                if self.server_conn.is_authenticated() {
                    match ty {
                        EventType::SendLogs => {
                            println!("panel asked for logs...");

                            self.server_conn.send_if_authenticated(|| {
                                PanelMessage::Command("silly".to_owned())
                            })
                        }

                        EventType::SendStats => {
                            println!("panel asked for starts");
                        }

                        _ => {}
                    }
                }

                Some(())
            },
        }
    }
}
