use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct BridgeMessage {
    client_name: Box<String>,
    group: Box<String>,
    port: u32,
    message: Box<String>,
}