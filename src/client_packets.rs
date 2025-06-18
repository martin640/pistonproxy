use crate::packet::{MinecraftPacket, MinecraftProtocolState, PacketParseError};
use crate::reader::{CursoredVarDataReader};
use crate::writer::CursoredVarDataWriter;

pub struct HandshakePacket {
    pub protocol_version: u32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: MinecraftProtocolState
}

impl TryFrom<&mut MinecraftPacket> for HandshakePacket {
    type Error = PacketParseError;
    
    fn try_from(packet: &mut MinecraftPacket) -> Result<Self, Self::Error> {
        CursoredVarDataReader::reset_cursor(packet);
        let f1 = packet.read_int().ok_or(PacketParseError::MalformedField(String::from("protocol_version")))?;
        let f2 = packet.read_string().ok_or(PacketParseError::MalformedField(String::from("server_address")))?;
        let f3 = packet.read_u16().ok_or(PacketParseError::MalformedField(String::from("server_port")))?;
        let f4 = packet.read_int().ok_or(PacketParseError::MalformedField(String::from("next_state")))?;
        Ok(HandshakePacket {
            protocol_version: f1 as u32,
            server_address: f2,
            server_port: f3,
            next_state: MinecraftProtocolState::from(f4 as usize)
        })
    }
}

impl From<HandshakePacket> for MinecraftPacket {
    fn from(value: HandshakePacket) -> Self {
        let mut packet = MinecraftPacket::empty();
        packet.write_int(value.protocol_version as i32);
        packet.write_string(&value.server_address);
        packet.write_u16(value.server_port);
        let next_state: usize = value.next_state.into();
        packet.write_int(next_state as i32);
        
        packet
    }
}