use crate::packet::{CONTINUE_BIT, SEGMENT_BITS};

pub trait VarDataWriter {
    fn write_int(&mut self, val: i32, offset: usize) -> usize;
    
    fn write_long(&mut self, val: i64, offset: usize) -> usize;
    
    fn write_u16(&mut self, val: u16, offset: usize);
    
    fn write_string(&mut self, val: &String, offset: usize) -> usize;
}

impl VarDataWriter for Vec<u8> {
    fn write_int(&mut self, val: i32, offset: usize) -> usize {
        let mut value = val;
        let mut cursor = offset;
        let mut terminate: bool;
        let mut next_byte: u8;
        
        loop {
            terminate = (value & !(SEGMENT_BITS as i32)) == 0;
            
            if terminate {
                next_byte = value as u8;
            } else {
                next_byte = ((value & (SEGMENT_BITS as i32)) | (CONTINUE_BIT as i32)) as u8;
            }
            
            if self.len() == cursor {
                self.push(next_byte);
            } else {
                self[cursor] = next_byte;
            }
            cursor += 1;
            
            if terminate {
                break;
            } else {
                value = ((value as u32) >> 7) as i32;
            }
        }
        
        cursor - offset
    }
    
    fn write_long(&mut self, val: i64, offset: usize) -> usize {
        let mut value = val;
        let mut cursor = offset;
        let mut terminate: bool;
        let mut next_byte: u8;
        
        loop {
            terminate = (value & !(SEGMENT_BITS as i64)) == 0;
            
            if terminate {
                next_byte = value as u8;
            } else {
                next_byte = ((value & (SEGMENT_BITS as i64)) | (CONTINUE_BIT as i64)) as u8;
            }
            
            if self.len() == cursor {
                self.push(next_byte);
            } else {
                self[cursor] = next_byte;
            }
            cursor += 1;
            
            if terminate {
                break;
            } else {
                value = ((value as u64) >> 7) as i64;
            }
        }
        
        cursor - offset
    }
    
    fn write_u16(&mut self, val: u16, offset: usize) {
        if self.len() < offset + 2 {
            self.resize(offset + 2, 0);
        }
        let bytes = val.to_be_bytes();
        self[offset] = bytes[0];
        self[offset + 1] = bytes[1];
    }
    
    fn write_string(&mut self, val: &String, offset: usize) -> usize {
        let bytes = val.as_bytes();
        let prefix_len = self.write_int(bytes.len() as i32, offset);
        let total_len = prefix_len + bytes.len();
        if self.len() < offset + total_len {
            self.resize(offset + total_len, 0);
        }
        self[(offset + prefix_len)..(offset + total_len)].copy_from_slice(bytes);
        total_len
    }
}

pub trait CursoredVarDataWriter {
    fn reset_cursor(&mut self);
    
    fn write_int(&mut self, val: i32);
    
    fn write_long(&mut self, val: i64);
    
    fn write_u16(&mut self, val: u16);
    
    fn write_string(&mut self, val: &String);
}

#[cfg(test)]
mod tests {
    use std::io::{stdout, Write};
    use crate::reader::VarDataReader;
    use super::*;
    
    #[test]
    fn check_encoding_int() {
        let mut vec: Vec<u8> = Vec::new();
        let nums: [i32; 7] = [ 0, 100, -100, 255, -255, i32::MIN, i32::MAX ];
        
        nums.iter().for_each(|num| {
            println!("testing number {}", num);
            stdout().flush().unwrap();
            vec.write_int(*num, 0);
            let (decoded, _) = vec.read_int(0).unwrap();
            assert_eq!(*num, decoded);
        });
    }
    
    #[test]
    fn check_encoding_long() {
        let mut vec: Vec<u8> = Vec::new();
        let nums: [i64; 9] = [ 0, 100, -100, 255, -255, 99999999999999, -99999999999999, i64::MIN, i64::MAX ];
        
        nums.iter().for_each(|num| {
            println!("testing number {}", num);
            stdout().flush().unwrap();
            vec.write_long(*num, 0);
            let (decoded, _) = vec.read_long(0).unwrap();
            assert_eq!(*num, decoded);
        });
    }
    
    #[test]
    fn check_encoding_short() {
        let mut vec: Vec<u8> = Vec::new();
        let nums: [u16; 6] = [ 0, 100, 255, 127, 123, 72 ];
        
        nums.iter().for_each(|num| {
            println!("testing number {}", num);
            stdout().flush().unwrap();
            vec.write_u16(*num, 0);
            let decoded = vec.read_u16(0).unwrap();
            assert_eq!(*num, decoded);
        });
    }
    
    #[test]
    fn check_encoding_string() {
        let mut vec: Vec<u8> = Vec::new();
        let strings: [String; 3] = [
            String::from("hello world!"),
            String::from(""),
            String::from("123"),
        ];
        
        strings.iter().for_each(|str| {
            println!("testing string {}", str);
            stdout().flush().unwrap();
            vec.write_string(str, 0);
            let (decoded, _) = vec.read_string(0).unwrap();
            assert_eq!(*str, decoded);
        });
    }
}