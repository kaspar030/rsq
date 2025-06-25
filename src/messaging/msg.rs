use super::channel::ChannelId;
use super::peer::PeerId;
use super::util::hash;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Clone, Debug, Eq, Hash, Serialize, Deserialize, Encode)]
pub enum Msg {
    ChannelMsg(ChannelMsg),
    ControlMsg(ControlMsg),
    StatusMsg(StatusMsg),
}

impl Decode<bool> for Msg {
    fn decode<D: bincode::de::Decoder<Context = bool>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let variant_index = <u32 as ::bincode::Decode<bool>>::decode(decoder)?;
        match variant_index {
            0u32 => core::result::Result::Ok(Self::ChannelMsg {
                0: ::bincode::Decode::<bool>::decode(decoder)?,
            }),
            1u32 => core::result::Result::Ok(Self::ControlMsg {
                0: ::bincode::Decode::<bool>::decode(decoder)?,
            }),
            2u32 => core::result::Result::Ok(Self::StatusMsg {
                0: ::bincode::Decode::<bool>::decode(decoder)?,
            }),
            variant => {
                core::result::Result::Err(::bincode::error::DecodeError::UnexpectedVariant {
                    found: variant,
                    type_name: "Msg",
                    allowed: &::bincode::error::AllowedEnumVariants::Range { min: 0, max: 2 },
                })
            }
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode)]
pub struct ChannelMsg {
    sender: PeerId,
    channel: ChannelId,
    content: Vec<u8>,
}

impl Decode<bool> for ChannelMsg {
    fn decode<D: bincode::de::Decoder<Context = bool>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        if *decoder.context() {
            let res = ChannelMsg {
                sender: ::bincode::Decode::<bool>::decode(decoder)?,
                channel: ::bincode::Decode::<bool>::decode(decoder)?,
                content: vec![],
            };
            Ok(res)
        } else {
            let res = ChannelMsg {
                sender: ::bincode::Decode::<bool>::decode(decoder)?,
                channel: ::bincode::Decode::<bool>::decode(decoder)?,
                content: ::bincode::Decode::<bool>::decode(decoder)?,
            };
            Ok(res)
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct ChannelMsgHdr {
    sender: PeerId,
    channel: ChannelId,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ControlMsg {
    ChannelJoin(String),
    ChannelCreate(String),
    ChannelLeave(ChannelId),
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum StatusMsg {
    Connecting,
    Connected,
    Disconnected,
    ChannelId(String, ChannelId),
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
    pub fn channel(&self) -> ChannelId {
        self.channel
    }
    pub fn content(&self) -> &Vec<u8> {
        &self.content
    }
}

impl Msg {
    pub fn new_channel_msg(sender: PeerId, channel: ChannelId, content: Vec<u8>) -> Self {
        Self::ChannelMsg(ChannelMsg::new(sender, channel, content))
    }

    pub fn channel_join(channel_name: String) -> Self {
        Self::ControlMsg(ControlMsg::ChannelJoin(channel_name))
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
