use std::convert::Infallible;

use actix::{Actor, ActorContext, Addr, Handler, StreamHandler};
use actix_web_actors::ws;
use bytestring::ByteString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use smallvec::{smallvec, SmallVec};

use crate::config::AlerionConfig;

use super::auth::Auth;
use super::relay::ServerConnection;

macro_rules! impl_infallible_message {
    ($msg_ty:ty) => {
        impl actix::Message for $msg_ty {
            type Result = std::result::Result<(), std::convert::Infallible>;
        }
    };
}

#[derive(Copy, Clone, Debug, Serialize, PartialEq, Eq)]
pub enum ServerStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "stopping")]
    Stopping,
    #[serde(rename = "offline")]
    Offline,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkStatistics {
    pub rx_bytes: usize,
    pub tx_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceStatisics {
    pub memory_bytes: usize,
    pub memory_limit_bytes: usize,
    pub cpu_absolute: f64,
    pub network: NetworkStatistics,
    pub uptime: u64,
    pub state: ServerStatus,
    pub disk_bytes: usize,
}

#[derive(Debug, Clone)]
pub enum ServerMessage {
    Kill,
    Logs(String),
    Stats(PerformanceStatisics),
}

#[derive(Debug, Clone)]
pub enum PanelMessage {
    Command(String),
    ReceiveLogs,
    ReceiveStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawMessage {
    event: EventType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    args: Option<SmallVec<[serde_json::Value; 1]>>,
}

impl From<RawMessage> for ByteString {
    fn from(value: RawMessage) -> Self {
        // there is no way this could fail, right
        serde_json::to_string(&value).unwrap().into()
    }
}

impl RawMessage {
    pub fn new_no_args(event: EventType) -> Self {
        Self { event, args: None }
    }

    pub fn new(event: EventType, args: String) -> Self {
        Self { event, args: Some(smallvec![serde_json::Value::String(args)]) }
    }

    pub fn into_first_arg(self) -> Option<String> {
        let mut args = self.args?;
        let json_str = args.get_mut(0)?.take();

        match json_str {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn event(&self) -> EventType {
        self.event
    }

    pub fn into_args(self) -> Option<SmallVec<[serde_json::Value; 1]>> {
        self.args
    }
}

impl_infallible_message!(ServerMessage);
impl_infallible_message!(PanelMessage);
impl_infallible_message!(RawMessage);

#[derive(Debug, Default)]
struct Permissions {
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum EventType {
    #[serde(rename = "auth")]
    Authentication,
    #[serde(rename = "auth success")]
    AuthenticationSuccess,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "logs")]
    Logs,
    #[serde(rename = "console output")]
    ConsoleOutput,
    #[serde(rename = "install output")]
    InstallOutput,
    #[serde(rename = "install completed")]
    InstallCompleted,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "send logs")]
    SendLogs,
    #[serde(rename = "send stats")]
    SendStats,
    #[serde(rename = "send command")]
    SendCommand,
    #[serde(rename = "set state")]
    SetState,
    #[serde(rename = "daemon error")]
    DaemonError,
    #[serde(rename = "jwt error")]
    JwtError,
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
                ctx.text(RawMessage::new(EventType::Stats, serde_json::to_string(&stats).unwrap()))
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
                if self
                    .auth
                    .is_valid(&event.into_first_arg()?, &self.server_uuid)
                {
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
