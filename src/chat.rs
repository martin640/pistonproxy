use std::fmt::Display;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize)]
pub struct ChatData {
    text: String,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<Vec<ChatData>>
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
}

impl Display for ChatData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", json!(self).to_string())
    }
}
