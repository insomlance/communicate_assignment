use std::{
    collections::HashMap,
    error::Error,
    hash::Hash,
    sync::{Arc, Mutex},
    thread,
};

use common::{data::BridgeMessage, get_runtime, parse_message_list, verify};
use log::{debug, error, info};
use rsa::RsaPublicKey;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{
        broadcast::{self},
        mpsc::{self, Receiver, Sender},
    },
};

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

pub struct RegisterInfo
//where
//    T: Send + 'static + Serialize + DeserializeOwned+ Router<String> + Message + Clone,
{
    pub addr: Box<String>,
    pub name: Box<String>,
    pub group: Box<String>,
    //_marker: PhantomData<T>,
}

impl RegisterInfo
//where T: Send + 'static + Serialize + DeserializeOwned+ Router<String> + Message + Clone,
{
    fn get_source_id(&self) -> Box<String> {
        let ans: String = self.name.to_string() + &(self.group.to_string());
        Box::new(ans)
    }
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

pub trait Message {
    fn set_error_msg(&mut self, error_msg: Box<String>);

    fn get_signature(&self) -> Option<&Vec<u8>>;
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

type RouteTable<M> = Arc<Mutex<HashMap<String, BcMsgSender<M>>>>;
type PubKeyTable = Arc<Mutex<HashMap<String, RsaPublicKey>>>;
type BcMsgSender<M> = tokio::sync::broadcast::Sender<M>;
type BcMsgReceiver<M> = tokio::sync::broadcast::Receiver<M>;

pub async fn listen_relayer_register<T>(
    mut clients_rx: Receiver<RegisterInfo>,
    route_table: RouteTable<T>,
    pub_keys: PubKeyTable,
) -> Result<(), String>
where
    T: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    let future = tokio::spawn(async move {
        while let Some(register_info) = clients_rx.recv().await {
            let route_table = route_table.clone();
            let pub_keys = pub_keys.clone();
            info!("relayer have receive new register={}", register_info.addr);
            tokio::spawn(async move {
                let res = relayer_connect(
                    &register_info.addr,
                    route_table,
                    pub_keys,
                    *register_info.get_source_id(),
                )
                .await;
                if let Err(error) = res {
                    error!(
                        "relayer error to listen to addr: {},error = {}",
                        &register_info.addr, error
                    );
                } else {
                    println!(
                        "success register in relayer end, addr={}",
                        &register_info.addr
                    );
                }
            });
        }
    })
    .await;
    if let Err(error) = future {
        error!(
            "error happen when listen clients to register bind ,error = {}",
            error
        );
        return Err("get relayer register error".to_string());
    }
    Ok(())
}

async fn relayer_connect<T>(
    addr: &str,
    route_table: RouteTable<T>,
    pub_keys: PubKeyTable,
    identity: String,
) -> Result<(), Box<dyn Error>>
where
    T: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    let stream = TcpStream::connect(addr).await?;
    let (reader, writer) = stream.into_split();

    debug!("has connect to addr={}", addr);

    let (send_tx, send_rx): (BcMsgSender<T>, BcMsgReceiver<T>) = broadcast::channel(16);

    {
        let mut lock_table = route_table.lock().map_err(|err| err.to_string())?;
        lock_table.insert(identity, send_tx);
    }

    tokio::spawn(async move {
        do_send(send_rx, writer).await;
    });
    tokio::spawn(async move {
        do_receive(route_table, pub_keys, reader).await;
    });
    Ok(())
}

async fn do_receive<T>(route_table: RouteTable<T>, pub_keys: PubKeyTable, mut reader: OwnedReadHalf)
where
    T: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    let mut buf = [0; 4096];
    while let Ok(size) = reader.read(&mut buf).await {
        if size == 0 {
            continue;
        }
        let serialized = String::from_utf8_lossy(&mut buf[0..size]);
        debug!("relayer receive message={}", serialized);
        let mut item_list = parse_message_list::<T>(&serialized);
        while !item_list.is_empty() {
            let parsed = item_list.remove(0);
            if let Err(error) = transfer_msg(route_table.clone(), pub_keys.clone(), parsed) {
                error!("transfer msg failed,msg={},error={}", serialized, error);
            }
        }
    }
}

fn transfer_msg<T>(
    route_table: RouteTable<T>,
    pub_keys: PubKeyTable,
    mut parsed: T,
) -> Result<(), String>
where
    T: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    let id = parsed.get_target_id();
    let source_id = parsed.get_source_id();
    let mut route_table = route_table.lock().map_err(|err| err.to_string())?;
    let pub_keys = pub_keys.lock().map_err(|err| err.to_string())?;
    verify_signature(&parsed, pub_keys.get(&source_id))?;
    if let Some(sender) = route_table.get_mut(&id) {
        channel_send(sender, parsed)
    } else {
        let sender = route_table
            .get_mut(&source_id)
            .ok_or("relayer can't find both source and target")?;
        parsed.set_error_msg(Box::new("can't find target".to_string()));
        channel_send(sender, parsed)
    }
    Ok(())
}

fn verify_signature<T>(item: &T, public_key: Option<&RsaPublicKey>) -> Result<(), String>
where
    T: Message + Router<String>,
{
    let public_key = public_key.ok_or("miss public key")?;
    let sign = item.get_signature().ok_or("miss signature")?;
    verify(&item.get_source_id(), public_key, sign)?;
    Ok(())
}

fn channel_send<T>(sender: &BcMsgSender<T>, item: T) {
    if let Err(error) = sender.send(item) {
        error!("relayer channel transfer failed,error={}", error);
    }
}

async fn do_send<T>(mut input: BcMsgReceiver<T>, mut writer: OwnedWriteHalf)
where
    T: Send + 'static + Serialize + DeserializeOwned + Clone,
{
    while let Ok(raw_msg) = input.recv().await {
        let res = serde_json::to_string(&raw_msg);
        match res {
            Ok(mut serialized) => {
                serialized.push_str("/*1^/");
                if let Err(error) = writer.write_all(serialized.as_bytes()).await {
                    error!("relayer sender error to write to stream; error = {}", error);
                }
            }
            Err(error) => error!("relayer sender serialize message error,error={}", error),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
