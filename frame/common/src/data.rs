use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use tokio::net::tcp::OwnedWriteHalf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeMessage {
    pub from_name: Box<String>,
    pub from_group: Box<String>,
    pub to_name: Box<String>,
    pub to_group: Box<String>,
    pub message: Box<String>,
    pub error_msg: Option<Box<String>>,
    pub sig:Option<Vec<u8>>,
}
pub trait Message {
    fn set_error_msg(&mut self, error_msg: Box<String>);

    fn get_signature(&self) -> Option<&Vec<u8>>;
}

pub trait Router<ID>
where
    ID: Eq + Hash,
{
    fn get_source_id(&self) -> ID;
    fn get_target_id(&self) -> ID;

    fn get_source_stream<'a>(
        &self,
        route_table: &'a HashMap<ID, OwnedWriteHalf>,
    ) -> Option<&'a OwnedWriteHalf> {
        route_table.get(&self.get_source_id())
    }

    fn get_target_stream<'a>(
        &self,
        route_table: &'a HashMap<ID, OwnedWriteHalf>,
    ) -> Option<&'a OwnedWriteHalf> {
        route_table.get(&self.get_target_id())
    }
}

impl Router<String> for BridgeMessage {
    fn get_source_id(&self) -> String {
        let ans: String = self.from_name.to_string() + &(self.from_group.to_string());
        ans
    }

    fn get_target_id(&self) -> String {
        let ans: String = self.to_name.to_string() + &(self.to_group.to_string());
        ans
    }
}

impl Message for BridgeMessage {
    fn set_error_msg(&mut self, error_msg: Box<String>) {
        self.error_msg = Some(error_msg);
    }

    fn get_signature(&self) -> Option<&Vec<u8>> {
        self.sig.as_ref()
    }
}
