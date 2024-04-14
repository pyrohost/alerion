macro_rules! impl_infallible_message {
    ($msg_ty:ty) => {
        impl actix::Message for $msg_ty {
            type Result = std::result::Result<(), std::convert::Infallible>;
        }
    }
}

impl_infallible_message!(ServerMessage);
impl_infallible_message!(PanelMessage);

#[derive(Debug)]
pub enum ServerMessage {
    Kill,
}

#[derive(Debug)]
pub enum PanelMessage {
    Command(String),
}

