use super::channel::ChannelId;
use super::peer::PeerId;
use super::util::hash;

use bincode::{BorrowDecode, Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Debug, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
pub enum Msg {
    ChannelMsg(ChannelMsg),
    ControlMsg(ControlMsg),
    StatusMsg(StatusMsg),
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode)]
pub struct ChannelMsg {
    sender: PeerId,
    channel: ChannelId,
    content: Vec<u8>,
}

impl<'a, Context> BorrowDecode<'a, Context> for ChannelMsg {
    fn borrow_decode<D: bincode::de::Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let ChannelMsgHdr { sender, channel } = ChannelMsgHdr::decode(decoder)?;

        let res = ChannelMsg {
            sender,
            channel,
            content: vec![],
        };

        Ok(res)
    }
}

impl<Context> Decode<Context> for ChannelMsg {
    fn decode<D: bincode::de::Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let ChannelMsgHdr { sender, channel } = ChannelMsgHdr::decode(decoder)?;

        let res = ChannelMsg {
            sender,
            channel,
            content: vec![],
        };

        Ok(res)
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct ChannelMsgHdr {
    sender: PeerId,
    channel: ChannelId,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ControlMsg {
    ChannelJoin(ChannelId),
    ChannelLeave(ChannelId),
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum StatusMsg {
    Connecting,
    Connected,
    Disconnected,
}

impl ChannelMsg {
    pub fn new(sender: PeerId, channel: ChannelId, content: Vec<u8>) -> Self {
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
    pub fn content(&self) -> &Vec<u8> {
        &self.content
    }
}

impl Msg {
    pub fn new_channel_msg(sender: PeerId, channel: ChannelId, content: Vec<u8>) -> Self {
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
