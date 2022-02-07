use std::{sync::{Arc, Mutex}, collections::HashMap, thread};

use frame_common::{data::{Router, Message}, get_runtime};
use frame_relayer::{RouteTable, PubKeyTable, RegisterInfo, listen_relayer_register};
use rsa::RsaPublicKey;
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::mpsc::{Sender, Receiver, self};

pub struct Relayer<Contract>
where
    Contract: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    route_table: Option<RouteTable<Contract>>,
    pub_keys: Option<PubKeyTable>,
    register: Option<Sender<RegisterInfo>>,
}

impl<Contract> Relayer<Contract>
where
    Contract: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    pub fn new() -> Relayer<Contract> {
        Relayer {
            route_table: None,
            pub_keys: None,
            register: None,
        }
    }
    pub fn launch(&mut self) {
        self.route_table = Some(Arc::new(Mutex::new(HashMap::new())));
        self.pub_keys = Some(Arc::new(Mutex::new(HashMap::new())));
        let clone_route_table = self.route_table.as_ref().unwrap().clone();
        let clone_pub_keys = self.pub_keys.as_ref().unwrap().clone();
        let rt = get_runtime();
        let (relayer_register_tx, relayer_register_rx): (
            Sender<RegisterInfo>,
            Receiver<RegisterInfo>,
        ) = mpsc::channel(32);
        thread::spawn(move || {
            rt.block_on(listen_relayer_register::<Contract>(
                relayer_register_rx,
                clone_route_table,
                clone_pub_keys,
            ))
        });
        self.register = Some(relayer_register_tx);
    }

    pub fn is_ready(&self) -> bool {
        match self.route_table {
            None => return false,
            _ => (),
        }
        match self.pub_keys {
            None => return false,
            _ => (),
        }
        match self.register {
            None => return false,
            _ => (),
        }
        true
    }

    pub async fn register_node(
        &self,
        register_info: RegisterInfo,
        pub_key: RsaPublicKey,
    ) -> Result<(), String> {
        match &self.pub_keys {
            Some(pub_key_map) => {
                let mut lock = pub_key_map.lock().map_err(|err| err.to_string())?;
                lock.insert((&register_info.get_source_id()).to_string(), pub_key);
            }
            None => return Err("relayer not ready".to_string()),
        }
        match &self.register {
            Some(register) => {
                register
                    .send(register_info)
                    .await
                    .map_err(|err| err.to_string())?;
            }
            None => return Err("register not exist".to_string()),
        }
        Ok(())
    }
}
