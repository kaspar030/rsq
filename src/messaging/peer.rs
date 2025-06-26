use bincode::{Decode, Encode};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

pub type PeerTx = flume::Sender<Bytes>;
pub type PeerRx = flume::Receiver<Bytes>;

#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, Encode, Decode)]
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
    fn get_sink(&self) -> &PeerTx;
}

unsafe impl Send for PeerId {}
