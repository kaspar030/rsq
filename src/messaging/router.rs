use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::channel::{Channel, ChannelId};
use super::msg::{ChannelMsg, Msg};
use super::peer::{Peer, PeerHandle, PeerId};
use anyhow::{anyhow, Error};
use std::collections::HashMap;
use std::sync::Arc;

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

    pub fn forward(&mut self, msg: Arc<Msg>, sender: &PeerId) -> Result<(), Error> {
        let (channel, channel_id) = if let Msg::ChannelMsg(msg) = msg.as_ref() {
            (self.channels.get_mut(msg.channel()), msg.channel())
        } else {
            unreachable!();
        };

        if let Some(channel) = channel {
            channel.forward(msg, sender);
            Ok(())
        } else {
            Err(anyhow!("unknown channel {:?}", channel_id))
        }
    }

    pub fn attach(&mut self, channel_id: &ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self.channels.entry(channel_id.clone()).or_insert_with(|| {
            tracing::info!("creating channel {}", channel_id.0);
            Channel::new(channel_id.clone())
        });

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
