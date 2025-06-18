use crate::packet::{CONTINUE_BIT, SEGMENT_BITS};

pub trait VarDataReader {
    fn read_int(&self, offset: usize) -> Option<(i32, usize)>;
    
    fn read_long(&self, offset: usize) -> Option<(i64, usize)>;
    
    fn read_u16(&self, offset: usize) -> Option<u16>;
    
    fn read_string(&self, offset: usize) -> Option<(String, usize)>;
}

impl VarDataReader for Vec<u8> {
    fn read_int(&self, offset: usize) -> Option<(i32, usize)> {
        let mut value: i32 = 0;
        let mut position: usize = 0;
        let mut cursor = offset;
        
        loop {
            if position >= 32 || cursor >= self.len() {
                return None
            }
            
            let current_byte: i32 = self[cursor] as i32;
            let next: i32 = (current_byte & (SEGMENT_BITS as i32)) << position;
            value = value | next;
            
            position = position + 7;
            cursor += 1;
            
            if (current_byte & (CONTINUE_BIT as i32)) == 0 {
                break;
            }
        }
        
        Some((value, cursor - offset))
    }
    
    fn read_long(&self, offset: usize) -> Option<(i64, usize)> {
        let mut value: i64 = 0;
        let mut position: usize = 0;
        let mut cursor = offset;
        
        loop {
            let current_byte: i64 = self[cursor] as i64;
            let next: i64 = (current_byte & (SEGMENT_BITS as i64)) << position;
            value = value | next;
            
            position = position + 7;
            cursor += 1;
            
            if (current_byte & (CONTINUE_BIT as i64)) == 0 {
                break;
            }
            
            if position >= 64 || cursor >= self.len() {
                return None
            }
        }
        
        Some((value, cursor - offset))
    }
    
    fn read_u16(&self, offset: usize) -> Option<u16> {
        if offset + 2 <= self.len() {
            let bytes: [u8; 2] = [
                self[offset],
                self[offset + 1]
            ];
            let val = u16::from_be_bytes(bytes);
            Some(val)
        } else {
            None
        }
    }
    
    fn read_string(&self, offset: usize) -> Option<(String, usize)> {
        match self.read_int(offset) {
            None => None,
            Some((str_len, prefix_len)) => {
                let start = offset + prefix_len;
                let end = start + (str_len as usize);
                let slice = self[start..end].to_vec();
                match String::from_utf8(slice) {
                    Ok(str) => Some((str, prefix_len + (str_len as usize))),
                    Err(_) => Some((String::new(), prefix_len + (str_len as usize))),
                }
            }
        }
    }
}

pub trait CursoredVarDataReader {
    fn reset_cursor(&mut self);
    
    fn read_int(&mut self) -> Option<i32>;
    
    fn read_long(&mut self) -> Option<i64>;
    
    fn read_u16(&mut self) -> Option<u16>;
    
    fn read_string(&mut self) -> Option<String>;
}