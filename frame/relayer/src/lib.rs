use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex}, error::Error,
};

use common::{parse_message_list, data::BridgeMessage};
use log::{debug, error, info};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream},
    sync::{broadcast::{Sender, self}, mpsc::Receiver},
};

struct RegisterInfo
//where
//    T: Send + 'static + Serialize + DeserializeOwned+ Router<String> + Message + Clone,
{
    addr: Box<String>,
    name: Box<String>,
    group: Box<String>,
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
}

type RouteTable<M> = Arc<Mutex<HashMap<String, BcMsgSender<M>>>>;
type BcMsgSender<M> = tokio::sync::broadcast::Sender<M>;
type BcMsgReceiver<M> = tokio::sync::broadcast::Receiver<M>;

async fn listen_relayer_register(mut clients_rx: Receiver<RegisterInfo>) -> Result<(), Box<dyn Error>> {
    let route_table: Arc<Mutex<HashMap<String, BcMsgSender<BridgeMessage>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let future=tokio::spawn(async move {
        while let Some(register_info) = clients_rx.recv().await {
            let route_table = route_table.clone();
            info!("relayer have receive new register={}", register_info.addr);
            tokio::spawn(async move {
                let res=relayer_connect(
                    &register_info.addr,
                    route_table,
                    *register_info.get_source_id(),
                )
                .await;
                if let Err(error) = res {
                    error!(
                        "relayer error to listen to addr: {},error = {}",
                        &register_info.addr, error
                    );
                } else {
                    println!("success register in relayer end, addr={}", &register_info.addr);
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
        return Err(Box::new(error));
    }
    Ok(())
}

async fn relayer_connect<T>(
    addr: &str,
    route_table: RouteTable<T>,
    identity: String,
) -> Result<(), Box<dyn Error>>
where
    T: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    let stream = TcpStream::connect(addr).await?;
    let (mut reader, mut writer) = stream.into_split();

    debug!("has connect to addr={}", addr);

    let (send_tx, send_rx): (BcMsgSender<T>, BcMsgReceiver<T>) = broadcast::channel(16);

    {
        let mut lock_table = route_table.lock().unwrap();
        lock_table.insert(identity, send_tx);
    }

    let write_future = tokio::spawn(async move {
        do_send(send_rx, writer).await;
    });
    let read_future = tokio::spawn(async move {
        do_receive::<T>(route_table, reader).await;
    });
    Ok(())
}

async fn do_receive<T>(mut route_table: RouteTable<T>, mut reader: OwnedReadHalf)
where
    T: Send + 'static + Serialize + DeserializeOwned + Router<String> + Message + Clone,
{
    let mut buf = [0; 1024];
    while let Ok(size) = reader.read(&mut buf).await {
        if size != 0 {
            let serialized = String::from_utf8_lossy(&mut buf[0..size]);
            debug!("relayer receive message={}", serialized);
            let mut item_list = parse_message_list::<T>(&serialized);
            while !item_list.is_empty() {
                let mut parsed = item_list.remove(0);
                let id = parsed.get_target_id();
                {
                    let mut route_table = route_table.lock().unwrap();
                    if let Some(sender) = route_table.get_mut(&id) {
                        channel_send(sender, parsed)
                    } else {
                        let source_id = parsed.get_source_id();
                        if let Some(sender) = route_table.get_mut(&source_id) {
                            parsed.set_error_msg(Box::new("can't find target".to_string()));
                            channel_send(sender, parsed)
                        } else {
                            error!(
                                "relayer can't find both source and target,msg={}",
                                serialized
                            );
                        }
                    }
                }
            }
        }
    }
}

fn channel_send<T>(mut sender: &Sender<T>, item: T) {
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
