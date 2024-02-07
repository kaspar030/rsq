use super::channel::{Channel, ChannelId};
use super::msg::Msg;
use super::peer::{Peer, PeerHandle, PeerId};
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Error;

use super::errors::TxError;

#[derive(Debug, Default)]
pub struct Router {
    peers: HashMap<PeerId, PeerHandle>,
    channels: HashMap<ChannelId, Channel>,
}

impl Router {
    pub fn new() -> Router {
        Router::default()
    }

    pub fn peer_add(&mut self, peer: &dyn Peer) {
        self.peers
            .insert(peer.get_id().clone(), peer.get_sink().clone());
    }

    pub fn peer_remove(&mut self, peer: &dyn Peer) {
        self.peers.remove(peer.get_id()).unwrap();
    }

    pub fn channel_add(&mut self, channel: Channel) {
        self.channels.insert(channel.get_id().clone(), channel);
    }

    pub fn channel_get(&mut self, channel_id: &ChannelId) -> Option<&mut Channel> {
        self.channels.get_mut(channel_id)
    }

    pub fn channel_get_or_add(&mut self, channel_id: &ChannelId) -> &mut Channel {
        self.channels.entry(channel_id.clone()).or_insert_with(|| {
            tracing::info!("creating channel {}", channel_id.0);
            Channel::new(channel_id.clone())
        })
    }

    pub fn forward(&mut self, msg: Arc<Msg>, sender: &PeerId) -> usize {
        let channel = if let Msg::ChannelMsg(msg) = msg.as_ref() {
            self.channel_get_or_add(msg.channel())
        } else {
            unreachable!();
        };

        channel.forward(msg, sender)
    }

    pub fn attach(&mut self, channel_id: &ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self.channel_get_or_add(channel_id);

        channel.subscribe(peer);

        Ok(())
    }

    pub fn detach(&mut self, channel_id: &ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self.channels.get_mut(channel_id);
        if let Some(channel) = channel {
            if channel.unsubscribe(peer).is_some() {
                tracing::info!("dropping channel {}", channel_id.0);
                self.channels.remove(channel_id);
            }
        }
        Ok(())
    }
}
