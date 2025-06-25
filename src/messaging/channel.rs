use bincode::{BorrowDecode, Decode, Encode};
use bytes::Bytes;
use slotmap::{new_key_type, KeyData};
use std::collections::HashMap;

use super::peer::{Peer, PeerHandle, PeerId};

new_key_type! {
    pub struct ChannelId;
}

impl Encode for ChannelId {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Encode::encode(&self.0.as_ffi(), encoder)
    }
}

impl<Context> Decode<Context> for ChannelId {
    fn decode<D: bincode::de::Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(ChannelId(KeyData::from_ffi(u64::decode(decoder)?)))
    }
}

impl<'de, Context> BorrowDecode<'de, Context> for ChannelId {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(ChannelId(KeyData::from_ffi(u64::borrow_decode(decoder)?)))
    }
}

#[derive(Debug)]
pub struct Channel {
    id: ChannelId,
    name: String,
    subscriptions: HashMap<PeerId, PeerHandle>,
}

impl Channel {
    pub fn new(name: String, id: ChannelId) -> Channel {
        Channel {
            id,
            name,
            subscriptions: HashMap::new(),
        }
    }

    pub fn get_id(&self) -> ChannelId {
        self.id
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub(crate) fn set_id(&mut self, id: ChannelId) {
        self.id = id
    }

    pub fn subscribe(&mut self, peer: &dyn Peer) {
        self.subscriptions
            .insert(peer.get_id().clone(), peer.get_sink().clone());
    }

    pub fn unsubscribe(&mut self, peer: &dyn Peer) -> Option<()> {
        self.subscriptions.remove(peer.get_id()).map(|_| ())
    }

    pub fn forward(&mut self, payload: Bytes, sender: &PeerId) -> usize {
        let mut count = 0usize;
        self.subscriptions.retain(|peer_id, peer| {
            if peer_id != sender {
                // If this errors, probably the peer's channel was closed when the peer
                // disconnected.
                peer.send(payload.clone()).inspect(|_| count += 1).is_ok()
            } else {
                true
            }
        });
        count
    }

    pub async fn forward_async(&mut self, payload: Bytes, sender: &PeerId) -> usize {
        let mut count = 0usize;
        let mut dropped = Vec::new();
        for (peer_id, peer) in &self.subscriptions {
            if peer_id != sender {
                // If this errors, probably the peer's channel was closed when the peer
                // disconnected.
                match peer.send_async(payload.clone()).await {
                    Ok(_) => {
                        count += 1;
                    }
                    Err(_) => {
                        dropped.push(peer_id.clone());
                    }
                }
            }
        }
        for peer_id in dropped {
            self.subscriptions.remove(&peer_id);
        }
        count
    }
}
