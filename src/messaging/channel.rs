use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::msg::{ChannelMsg, Msg};
use super::peer::{Peer, PeerHandle, PeerId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

#[derive(Debug)]
pub struct Channel {
    id: ChannelId,
    subscriptions: HashMap<PeerId, PeerHandle>,
}

impl Channel {
    pub fn new(id: ChannelId) -> Channel {
        Channel {
            id,
            subscriptions: HashMap::new(),
        }
    }

    pub fn get_id(&self) -> &ChannelId {
        &self.id
    }

    pub fn attach(&mut self, peer: &dyn Peer) {
        self.subscriptions
            .insert(peer.get_id().clone(), peer.get_sink().clone());
    }

    pub fn detach(&mut self, peer: &dyn Peer) -> Option<()> {
        self.subscriptions.remove(peer.get_id()).map(|_| ())
    }

    pub fn send(&mut self, msg: ChannelMsg) {
        self.subscriptions.retain(|peer_id, peer| {
            if peer_id != msg.sender() {
                match peer.send(Msg::ChannelMsg(msg.clone())) {
                    Ok(_) => true,
                    Err(_) => {
                        // probably the channel was closed when the peer
                        // disconnected.
                        false
                    }
                }
            } else {
                true
            }
        });
    }
}
