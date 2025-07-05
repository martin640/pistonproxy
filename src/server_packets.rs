use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::chat::ChatData;
use crate::packet::{MinecraftPacket};
use crate::writer::CursoredVarDataWriter;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServerVersion {
    pub name: String,
    pub protocol: i32
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServerPlayersInfo {
    pub max: i32,
    pub online: i32,
    pub sample: Vec<()>
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StatusPacket {
    pub version: ServerVersion,
    pub players: ServerPlayersInfo,
    pub description: ChatData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(rename = "enforces_secure_chat")]
    pub enforces_secure_chat: bool
}

impl ToString for StatusPacket {
    fn to_string(&self) -> String {
        json!(self).to_string()
    }
}

impl From<StatusPacket> for MinecraftPacket {
    fn from(value: StatusPacket) -> Self {
        let mut packet = MinecraftPacket::new(0x00);
        let json = value.to_string();
        packet.write_string(&json);
        packet
    }
}
