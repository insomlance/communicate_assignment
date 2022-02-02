use common::data::BridgeMessage;
use std::{
    error::Error,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use threadpool::ThreadPool;
use tokio::sync::mpsc::{error::SendError, Receiver, Sender};

use client::LaunchInfo;
use log::{debug, error, info};
use relayer::RegisterInfo;
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self},
};

use crate::receive_msg;

pub fn register_node(
    rt: &Runtime,
    name: &str,
    group: &str,
    addr: &str,
    client_register: Sender<LaunchInfo<BridgeMessage>>,
    relayer_register: Sender<RegisterInfo>,
    custom_task_register: Sender<CustomTaskInfo>,
    pool: Arc<Mutex<ThreadPool>>,
) -> Result<Sender<BridgeMessage>,String> {
    let (input_tx, input_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) = mpsc::channel(32);
    let (output_tx, output_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) =
        mpsc::channel(32);

    let register_info = build_register_info(name, group, addr);
    let client_register_info = build_client_register_info(name, addr, input_rx, output_tx);
    let custom_task_register_info = CustomTaskInfo {
        receiver: output_rx,
        pool: pool,
    };
    rt.block_on(async{
        register_all(
            client_register_info,
            client_register,
            register_info,
            relayer_register,
            custom_task_register_info,
            custom_task_register,
        ).await
    })?;

    Ok(input_tx)
}

async fn register_all(
    client_register_info: LaunchInfo<BridgeMessage>,
    client_register: Sender<LaunchInfo<BridgeMessage>>,
    register_info: RegisterInfo,
    relayer_register: Sender<RegisterInfo>,
    custom_task_register_info: CustomTaskInfo,
    custom_task_register: Sender<CustomTaskInfo>,
) -> Result<(), String> {
    client_register
        .send(client_register_info)
        .await
        .map_err(|err| err.to_string())?;
    thread::sleep(Duration::from_secs(1));
    relayer_register
        .send(register_info)
        .await
        .map_err(|err| err.to_string())?;
    custom_task_register
        .send(custom_task_register_info)
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

// fn convertSendError<T>(send_error:SendError<T>)->Result<(),String>{
//     Err("erer");
//     Ok(())
// }

fn build_register_info(name: &str, group: &str, addr: &str) -> RegisterInfo {
    RegisterInfo {
        addr: Box::new(addr.to_string()),
        name: Box::new(name.to_string()),
        group: Box::new(group.to_string()),
    }
}

fn build_client_register_info(
    name: &str,
    addr: &str,
    biz_input: Receiver<BridgeMessage>,
    biz_output: Sender<BridgeMessage>,
) -> LaunchInfo<BridgeMessage> {
    LaunchInfo {
        addr: Box::new(addr.to_string()),
        name: Box::new(name.to_string()),
        input: Box::new(biz_input),
        output: Box::new(biz_output),
    }
}

pub struct CustomTaskInfo {
    pub receiver: Receiver<BridgeMessage>,
    pub pool: Arc<Mutex<ThreadPool>>,
}

pub async fn listen_custom_tasks(
    mut receiver: Receiver<CustomTaskInfo>,
) -> Result<(), Box<dyn Error>>
where
{
    tokio::spawn(async move {
        while let Some(mut custom_task) = receiver.recv().await {
            tokio::spawn(async move {
                launch_custom_task(custom_task).await;
            });
        }
    })
    .await;
    Ok(())
}

async fn launch_custom_task(mut task: CustomTaskInfo) {
    while let Some(message) = task.receiver.recv().await {
        let mutex_pool = task.pool.lock().unwrap();
        receive_msg(message, mutex_pool);
    }
}
