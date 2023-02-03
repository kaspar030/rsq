use super::channel::ChannelId;
use super::peer::PeerId;
use super::util::hash;
use bytes::Bytes;

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Debug, Eq, Hash, Serialize, Deserialize)]
pub enum Msg {
    ChannelMsg(ChannelMsg),
    ControlMsg(ControlMsg),
    StatusMsg(StatusMsg),
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct ChannelMsg {
    sender: PeerId,
    channel: ChannelId,
    content: Bytes,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub enum ControlMsg {
    ChannelJoin(ChannelId),
    ChannelLeave(ChannelId),
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub enum StatusMsg {
    Connecting,
    Connected,
    Disconnected,
}

impl ChannelMsg {
    pub fn new(sender: PeerId, channel: ChannelId, content: Bytes) -> Self {
        Self {
            sender,
            channel,
            content,
        }
    }

    pub fn sender(&self) -> &PeerId {
        &self.sender
    }
    pub fn channel(&self) -> &ChannelId {
        &self.channel
    }
    pub fn content(&self) -> &Bytes {
        &self.content
    }
}

impl Msg {
    pub fn new_channel_msg(sender: PeerId, channel: ChannelId, content: Bytes) -> Self {
        Self::ChannelMsg(ChannelMsg::new(sender, channel, content))
    }

    pub fn channel_join(channel_id: ChannelId) -> Self {
        Self::ControlMsg(ControlMsg::ChannelJoin(channel_id))
    }

    pub fn channel_leave(channel_id: ChannelId) -> Self {
        Self::ControlMsg(ControlMsg::ChannelLeave(channel_id))
    }

    pub fn get_id(&self) -> MsgId {
        MsgId(hash(self))
    }

    pub fn new_status(status: StatusMsg) -> Self {
        Self::StatusMsg(status)
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct MsgId(u64);

mod test {
    //    use super::*;
}
