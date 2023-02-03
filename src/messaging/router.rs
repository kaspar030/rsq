use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::channel::{Channel, ChannelId};
use super::msg::{ChannelMsg, Msg};
use super::peer::{Peer, PeerHandle, PeerId};
use anyhow::{anyhow, Error};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Router {
    peers: HashMap<PeerId, PeerHandle>,
    channels: HashMap<ChannelId, Channel>,
}

impl Router {
    pub fn new() -> Router {
        Router {
            peers: HashMap::new(),
            channels: HashMap::new(),
        }
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

    pub fn send(&mut self, msg: ChannelMsg) -> Result<(), Error> {
        let mut channel = self.channels.get_mut(msg.channel());
        if let Some(channel) = &mut channel.as_mut() {
            channel.send(msg);
            Ok(())
        } else {
            Err(anyhow!("unknown channel {:?}", msg.channel()))
        }
    }

    pub fn attach(&mut self, channel_id: &ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self.channels.entry(channel_id.clone()).or_insert_with(|| {
            tracing::info!("creating channel {}", channel_id.0);
            Channel::new(channel_id.clone())
        });

        channel.attach(peer);

        Ok(())
    }

    pub fn detach(&mut self, channel_id: &ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self.channels.get_mut(channel_id);
        if let Some(channel) = channel {
            if channel.detach(peer).is_some() {
                tracing::info!("dropping channel {}", channel_id.0);
                self.channels.remove(channel_id);
            }
        }
        Ok(())
    }
}
