use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::msg::Msg;

pub type PeerHandle = flume::Sender<Arc<Msg>>;

#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub struct PeerId {
    id: String,
}

impl PeerId {
    pub fn new(id: &str) -> PeerId {
        PeerId { id: id.to_string() }
    }
}

pub trait Peer {
    fn get_id(&self) -> &PeerId;
    fn get_sink(&self) -> &PeerHandle;
}

unsafe impl Send for PeerId {}
