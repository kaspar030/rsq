use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::msg::Msg;
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

    pub fn subscribe(&mut self, peer: &dyn Peer) {
        self.subscriptions
            .insert(peer.get_id().clone(), peer.get_sink().clone());
    }

    pub fn unsubscribe(&mut self, peer: &dyn Peer) -> Option<()> {
        self.subscriptions.remove(peer.get_id()).map(|_| ())
    }

    pub fn forward(&mut self, msg: Arc<Msg>, sender: &PeerId) -> usize {
        let mut count = 0usize;
        self.subscriptions.retain(|peer_id, peer| {
            if peer_id != sender {
                // If this errors, probably the peer's channel was closed when the peer
                // disconnected.
                peer.send(msg.clone()).inspect(|_| count += 1).is_ok()
            } else {
                true
            }
        });
        count
    }
}
