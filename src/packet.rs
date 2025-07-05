use std::io::Write;
use crate::chat::ChatData;
use crate::reader::{CursoredVarDataReader, VarDataReader};
use crate::writer::{CursoredVarDataWriter, VarDataWriter};

pub const SEGMENT_BITS: u8 = 0x7F;
pub const CONTINUE_BIT: u8 = 0x80;

pub struct MinecraftPacket {
    pub len: i32,
    pub id: i32,
    pub data: Vec<u8>,
    cursor: usize
}

#[derive(Debug)]
pub enum PacketParseError {
    MalformedField(String),
    PacketFormatError(String),
    LengthMismatch,
    EmptyBuffer
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MinecraftProtocolState {
    HANDSHAKING,
    STATUS,
    LOGIN,
    PLAY,
    NONE
}

impl From<u16> for MinecraftProtocolState {
    fn from(val: u16) -> MinecraftProtocolState {
        match val {
            0 => MinecraftProtocolState::HANDSHAKING,
            1 => MinecraftProtocolState::STATUS,
            2 => MinecraftProtocolState::LOGIN,
            3 => MinecraftProtocolState::PLAY,
            _ => MinecraftProtocolState::NONE
        }
    }
}

impl Into<u16> for MinecraftProtocolState {
    fn into(self) -> u16 {
        match self {
            MinecraftProtocolState::HANDSHAKING => 0,
            MinecraftProtocolState::STATUS => 1,
            MinecraftProtocolState::LOGIN => 2,
            MinecraftProtocolState::PLAY => 3,
            MinecraftProtocolState::NONE => u16::MAX
        }
    }
}

impl MinecraftPacket {
    pub fn new(id: i32) -> MinecraftPacket {
        MinecraftPacket {
            len: 0,
            id,
            data: Vec::new(),
            cursor: 0
        }
    }
    
    pub fn parse_packet(buf: Vec<u8>) -> Result<(MinecraftPacket, usize), PacketParseError> {
        if buf.len() == 0 {
            return Err(PacketParseError::EmptyBuffer);
        }
        
        if buf.len() == 2 && buf[0] == 0xFE && buf[1] == 0x01 {
            return Ok((MinecraftPacket {
                len: 0,
                id: 255,
                cursor: 0,
                data: Vec::new()
            }, 2))
        }
        
        let mut offset = 0;
        if let Some((packet_length, length_len)) = buf.read_int(0) {
            offset += length_len;
            if let Some((packet_id, id_len)) = buf.read_int(offset) {
                offset += id_len;
                
                let total_length = length_len + (packet_length as usize);
                let data_length = (packet_length as usize) - id_len;
                
                if buf.len() >= total_length {
                    let data = buf[offset..(offset + data_length)].to_vec();
                    Ok((MinecraftPacket {
                        len: data_length as i32,
                        id: packet_id,
                        cursor: 0,
                        data
                    }, total_length))
                } else {
                    Err(PacketParseError::PacketFormatError(format!("expected packet of total size {} but buffer size is {}", total_length, buf.len())))
                }
            } else {
                Err(PacketParseError::PacketFormatError(String::from("unable to read packet id")))
            }
        } else {
            Err(PacketParseError::PacketFormatError(String::from("unable to read packet length")))
        }
    }
    
    pub fn create_disconnect_packet(msg: ChatData) -> MinecraftPacket {
        let mut packet = MinecraftPacket::new(0x00);
        let json = msg.to_string();
        packet.write_string(&json);
        packet
    }
    
    pub fn encode(&self) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        let packet_id_len = data.write_int(self.id, 0);
        let mut offset = data.write_int(self.len + (packet_id_len as i32), 0);
        offset += data.write_int(self.id, offset);
        data.resize(offset + (self.len as usize), 0);
        data[offset..(offset + (self.len as usize))].copy_from_slice(&self.data[..]);
        data
    }
}

impl CursoredVarDataReader for MinecraftPacket {
    fn reset_cursor(&mut self) {
        self.cursor = 0;
    }
    
    fn read_int(&mut self) -> Option<i32> {
        match self.data.read_int(self.cursor) {
            None => None,
            Some((val, len)) => {
                self.cursor = self.cursor + len;
                Some(val)
            }
        }
    }
    
    fn read_long(&mut self) -> Option<i64> {
        match self.data.read_long(self.cursor) {
            None => None,
            Some((val, len)) => {
                self.cursor = self.cursor + len;
                Some(val)
            }
        }
    }
    
    fn read_u16(&mut self) -> Option<u16> {
        match self.data.read_u16(self.cursor) {
            None => None,
            Some(val) => {
                self.cursor += 2;
                Some(val)
            }
        }
    }
    
    fn read_string(&mut self) -> Option<String> {
        match self.data.read_string(self.cursor) {
            None => None,
            Some((val, len)) => {
                self.cursor = self.cursor + len;
                Some(val)
            }
        }
    }
}

impl CursoredVarDataWriter for MinecraftPacket {
    fn reset_cursor(&mut self) {
        self.cursor = 0;
    }
    
    fn write_int(&mut self, val: i32) {
        let len = self.data.write_int(val, self.cursor);
        self.cursor += len;
        self.len = usize::max(self.len as usize, self.cursor) as i32;
    }
    
    fn write_long(&mut self, val: i64) {
        let len = self.data.write_long(val, self.cursor);
        self.cursor += len;
        self.len = usize::max(self.len as usize, self.cursor) as i32;
    }
    
    fn write_u16(&mut self, val: u16) {
        self.data.write_u16(val, self.cursor);
        self.cursor += 2;
        self.len = usize::max(self.len as usize, self.cursor) as i32;
    }
    
    fn write_string(&mut self, val: &String) {
        let len = self.data.write_string(val, self.cursor);
        self.cursor += len;
        self.len = usize::max(self.len as usize, self.cursor) as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn encode_and_decode_packet() {
        let msg = String::from("Hello world!");
        let original_packet = MinecraftPacket::create_disconnect_packet(ChatData::new(msg.clone()));
        let bytes = original_packet.encode();
        
        let res = MinecraftPacket::parse_packet(bytes.to_vec());
        let (mut parsed_packet, _) = res.unwrap();
        assert_eq!(parsed_packet.id, original_packet.id);
        let body = parsed_packet.read_string().unwrap();
        let chat_data = ChatData::try_from(body).unwrap();
        assert_eq!(chat_data.text, msg);
    }
}