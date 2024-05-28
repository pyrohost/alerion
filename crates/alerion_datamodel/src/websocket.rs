use bytestring::ByteString;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StateUpdate {
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "restart")]
    Restart,
    #[serde(rename = "kill")]
    Kill,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct RawMessage {
    event: EventType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    args: Option<SmallVec<[serde_json::Value; 1]>>,
}

impl From<RawMessage> for ByteString {
    fn from(value: RawMessage) -> Self {
        serde_json::to_string(&value)
            .expect("infallible struct-to-json conversion")
            .into()
    }
}

impl RawMessage {
    pub fn new_no_args(event: EventType) -> Self {
        Self { event, args: None }
    }

    pub fn new(event: EventType, args: String) -> Self {
        Self {
            event,
            args: Some(smallvec![serde_json::Value::String(args)]),
        }
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
