use std::convert::Infallible;
use serde::{Serialize, Deserialize};
use bytestring::ByteString;

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
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "install completed")]
    InstallCompleted,
    #[serde(rename = "send logs")]
    SendLogs,
    #[serde(rename = "send stats")]
    SendStats,
}

#[derive(Debug, Serialize)]
pub struct OutgoingEvent {
    event: EventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

impl actix::Message for OutgoingEvent {
    type Result = Result<(), Infallible>;
}

impl From<OutgoingEvent> for ByteString {
    fn from(value: OutgoingEvent) -> Self {
        // there is no way this could fail, right
        serde_json::to_string(&value).unwrap().into()
    }
}

impl OutgoingEvent {
    pub fn new_no_args(event: EventType) -> Self {
        OutgoingEvent {
            event,
            args: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct IncomingEvent {
    event: EventType,
    #[serde(default)]
    args: Option<serde_json::Value>,
}

impl IncomingEvent {
    pub fn try_parse(payload: &str) -> Option<Self> {
        serde_json::from_str(payload).ok()
    }

    pub fn into_first_arg_as_str(self) -> Option<String> {
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

    pub fn into_args(self) -> Option<serde_json::Value> {
        self.args
    }
}
