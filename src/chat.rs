use std::fmt::Display;
use serde::{Deserialize, Serialize};
use serde_json::{json};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatData {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<ChatData>>
}

impl ChatData {
    pub fn new(text: String) -> ChatData {
        ChatData {
            text,
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
            color: None,
            extra: None
        }
    }
    
    pub fn new_colored(text: String, color: String) -> ChatData {
        ChatData {
            text,
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
            color: Some(color),
            extra: None
        }
    }
}

impl Display for ChatData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", json!(self).to_string())
    }
}

impl TryFrom<String> for ChatData {
    type Error = ();
    
    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(&*value).map_err(|_| ())
    }
}
