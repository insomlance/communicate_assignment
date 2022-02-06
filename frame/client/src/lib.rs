#[cfg(test)]
mod tests;

use std::error::Error;

use frame_common::parse_message_list;
use log::{debug, error, info};
use serde::{de::DeserializeOwned, Serialize};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener,
    },
    sync::mpsc::{Receiver, Sender},
};

pub struct LaunchInfo<T>
where
    T: Send + 'static + Serialize + DeserializeOwned,
{
    pub name: Box<String>,
    pub addr: Box<String>,
    pub input: Box<Receiver<T>>,
    pub output: Box<Sender<T>>,
}

pub async fn listen_clients_register<T>(
    mut clients_rx: Receiver<LaunchInfo<T>>,
) -> Result<(), Box<dyn Error>>
where
    T: Send + 'static + Serialize + DeserializeOwned,
{
    let future = tokio::spawn(async move {
        while let Some(lauch_info) = clients_rx.recv().await {
            info!("client have receive new register={}", lauch_info.addr);
            tokio::spawn(async move {
                let res = client_listen(
                    &lauch_info.addr,
                    *lauch_info.input,
                    *lauch_info.output,
                    (&lauch_info.name).to_string(),
                )
                .await;
                if let Err(error) = res {
                    error!(
                        "client error to listen to addr: {},error = {}",
                        &lauch_info.addr, error
                    );
                } else {
                    println!("success register in client end, addr={}", &lauch_info.addr);
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

async fn client_listen<T>(
    addr: &str,
    input: Receiver<T>,
    output: Sender<T>,
    who: String,
) -> Result<(), Box<dyn Error>>
where
    T: Send + 'static + Serialize + DeserializeOwned,
{
    let listener = TcpListener::bind(addr).await?;
    let (stream, _) = listener.accept().await?;
    let (reader, writer) = stream.into_split();

    debug!("addr={} listen has build", addr);

    let who_clone = who.clone();
    tokio::spawn(async move {
        do_send(input, writer, who_clone).await;
    });
    let who_clone = who.clone();
    tokio::spawn(async move {
        do_receive(output, reader, who_clone).await;
    });
    Ok(())
}

async fn do_send<T>(mut input: Receiver<T>, mut writer: OwnedWriteHalf, who: String)
where
    T: Send + 'static + Serialize + DeserializeOwned,
{
    while let Some(raw_msg) = input.recv().await {
        let res = serde_json::to_string(&raw_msg);
        match res {
            Ok(mut serialized) => {
                serialized.push_str("/*1^/");
                debug!("sender get message={}", serialized);
                if let Err(error) = writer.write_all(serialized.as_bytes()).await {
                    error!("sender error to write to stream; error = {}", error);
                }
            }
            Err(error) => error!("sender serialize message error,error={}", error),
        }
    }
}

async fn do_receive<T>(output: Sender<T>, mut reader: OwnedReadHalf, who: String)
where
    T: Send + 'static + Serialize + DeserializeOwned,
{
    let mut buf = [0; 4096];
    while let Ok(size) = reader.read(&mut buf).await {
        if size != 0 {
            let serialized = String::from_utf8_lossy(&mut buf[0..size]);
            debug!("{} receiver get message={}", who, serialized);
            let mut item_list = parse_message_list::<T>(&serialized);
            while !item_list.is_empty() {
                let item = item_list.remove(0);
                if let Err(error) = output.send(item).await {
                    error!("receive then send out failed,error={}", error);
                }
            }
        }
    }
}
