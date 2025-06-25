use super::channel::{Channel, ChannelId};
use super::peer::{Peer, PeerHandle, PeerId};
use std::collections::HashMap;

use anyhow::Error;
use bytes::Bytes;
use slotmap::SlotMap;

use super::errors::TxError;

#[derive(Debug, Default)]
pub struct Router {
    peers: HashMap<PeerId, PeerHandle>,
    channels: SlotMap<ChannelId, Channel>,
    channel_names: HashMap<String, ChannelId>,
}

impl Router {
    pub fn new() -> Router {
        Router::default()
    }

    pub fn peer_add(&mut self, peer: &dyn Peer) {
        self.peers
            .insert(peer.get_id().clone(), peer.get_sink().clone());
    }

    pub fn peer_remove(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id).unwrap();
    }

    pub fn channel_get_or_add(&mut self, name: String) -> ChannelId {
        if let Some(key) = self.channel_names.get(&name) {
            return *key;
        }

        tracing::info!("creating channel {}", name);

        let key = self.channels.insert_with_key(|key| {
            let channel = Channel::new(name.clone(), key);
            channel
        });
        self.channel_names.insert(name, key);
        key
    }

    pub fn channel_get(&mut self, channel_id: ChannelId) -> Option<&mut Channel> {
        self.channels.get_mut(channel_id)
    }

    // pub fn channel_get_or_add(&mut self, channel_id: &ChannelId) -> &mut Channel {
    //     self.channels.entry(channel_id.clone()).or_insert_with(|| {
    //         tracing::info!("creating channel {}", channel_id.0);
    //         Channel::new(channel_id.clone())
    //     })
    // }

    pub fn forward(
        &mut self,
        payload: Bytes,
        channel_id: ChannelId,
        sender: &PeerId,
    ) -> Result<usize, TxError> {
        let channel = self
            .channel_get(channel_id)
            .ok_or(TxError::InvalidChannel)?;

        Ok(channel.forward(payload, sender))
    }

    pub async fn forward_async(
        &mut self,
        payload: Bytes,
        channel_id: ChannelId,
        sender: &PeerId,
    ) -> Result<usize, TxError> {
        let channel = self
            .channel_get(channel_id)
            .ok_or(TxError::InvalidChannel)?;

        Ok(channel.forward_async(payload, sender).await)
    }

    pub fn attach(&mut self, channel_id: ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self
            .channel_get(channel_id)
            .ok_or(TxError::InvalidChannel)?;

        channel.subscribe(peer);

        Ok(())
    }

    pub fn detach(&mut self, channel_id: ChannelId, peer: &dyn Peer) -> Result<(), Error> {
        let channel = self.channels.get_mut(channel_id);
        let mut remove = Option::default();
        if let Some(channel) = channel {
            if channel.unsubscribe(peer).is_some() {
                tracing::info!("dropping channel {}", channel.get_name());
                remove = Some(channel_id);
                self.channel_names.remove(channel.get_name());
            }
        }
        if let Some(channel_id) = remove {
            self.channels.remove(channel_id);
        }
        Ok(())
    }
}
