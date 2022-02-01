use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeMessage {
    pub from_name: Box<String>,
    pub from_group: Box<String>,
    pub to_name: Box<String>,
    pub to_group: Box<String>,
    pub message: Box<String>,
    pub error_msg: Option<Box<String>>,
}
