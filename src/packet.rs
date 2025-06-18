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
    LengthMismatch,
    EmptyBuffer
}

#[derive(Debug)]
pub enum MinecraftProtocolState {
    HANDSHAKING,
    STATUS,
    LOGIN,
    PLAY,
    NONE
}

impl From<usize> for MinecraftProtocolState {
    fn from(val: usize) -> MinecraftProtocolState {
        match val {
            0 => MinecraftProtocolState::HANDSHAKING,
            1 => MinecraftProtocolState::STATUS,
            2 => MinecraftProtocolState::LOGIN,
            3 => MinecraftProtocolState::PLAY,
            _ => MinecraftProtocolState::NONE
        }
    }
}

impl Into<usize> for MinecraftProtocolState {
    fn into(self) -> usize {
        match self {
            MinecraftProtocolState::HANDSHAKING => 0,
            MinecraftProtocolState::STATUS => 1,
            MinecraftProtocolState::LOGIN => 2,
            MinecraftProtocolState::PLAY => 3,
            MinecraftProtocolState::NONE => usize::MAX
        }
    }
}

impl MinecraftPacket {
    pub fn empty() -> MinecraftPacket {
        MinecraftPacket {
            len: 0,
            id: 0,
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
        if let Some((packet_length, len)) = buf.read_int(0) {
            offset = offset + len;
            if let Some((packet_id, len)) = buf.read_int(len) {
                offset = offset + len;
                let data_length = offset + packet_length as usize;
                
                assert_ne!(data_length, 0);
                
                if buf.len() >= data_length {
                    let data = buf[offset..data_length].to_vec();
                    Ok((MinecraftPacket {
                        len: packet_length,
                        id: packet_id,
                        cursor: 0,
                        data
                    }, data_length))
                } else {
                    Err(PacketParseError::LengthMismatch)
                }
            } else {
                Err(PacketParseError::LengthMismatch)
            }
        } else {
            Err(PacketParseError::LengthMismatch)
        }
    }
    
    pub fn create_disconnect_packet(msg: &String) -> MinecraftPacket {
        let mut packet = MinecraftPacket::empty();
        let json = ChatData::new(msg.clone()).to_string();
        packet.write_string(&json);
        packet
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
