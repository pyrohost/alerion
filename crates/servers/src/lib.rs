use actix::Addr;
use actix_web_actors::ws::Message;
use uuid::Uuid;
use alerion_websocket::WebsocketConnection;

pub struct InstallPool {
    
}

impl InstallPool {
    pub fn new() -> Self {
        InstallPool {}
    }

    pub fn push(&self, uuid: Uuid, addr: Addr<WebsocketConnection>) {
        tokio::spawn(async move {
            println!("what");
            let result = addr.try_send(alerion_websocket::Msg);
        });
    }
}
