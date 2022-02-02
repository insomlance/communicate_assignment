use common::get_runtime;
use common::data::BridgeMessage;
use threadpool::ThreadPool;
use tokio::sync::mpsc::{Receiver, Sender};
use std::{
    collections::HashMap,
    error::Error,
    io::{self, BufRead},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use client::{listen_clients_register, LaunchInfo};
use log::{debug, error, info};
use relayer::{listen_relayer_register, RegisterInfo};
use threadpool::{Builder};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self},
};

use crate::receive_msg;

pub fn register_node(
    name: &str,
    group: &str,
    addr: &str,
    client_register: Sender<LaunchInfo<BridgeMessage>>,
    relayer_register: Sender<RegisterInfo>,
    custom_task_register: Sender<CustomTaskInfo>,
    pool: Arc<Mutex<ThreadPool>>,
) -> Sender<BridgeMessage> {
    let (input_tx, input_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) = mpsc::channel(32);
    let (output_tx, output_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) =
        mpsc::channel(32);

    let rt = get_runtime();
    let register_info = build_register_info(name, group, addr);
    let client_register_info = build_client_register_info(name, group, addr, input_rx, output_tx);
    let custom_task_register_info = CustomTaskInfo {
        receiver: output_rx,
        pool: pool,
    };
    rt.block_on(async {
        register_all(
            client_register_info,
            client_register,
            register_info,
            relayer_register,
            custom_task_register_info,
            custom_task_register,
        )
        .await
    });

    input_tx
}

pub fn register_custom_tasks() -> Sender<CustomTaskInfo> {
    let (tx, mut rx) = mpsc::channel(8);
    let rt = get_runtime();
    thread::spawn(move || {
        rt.block_on(listen_custom_tasks(rx));
    });
    tx
}

pub fn get_relayer_register() -> Sender<RegisterInfo> {
    let rt = get_runtime();
    let (relayer_register_tx, relayer_register_rx): (Sender<RegisterInfo>, Receiver<RegisterInfo>) =
        mpsc::channel(32);
    thread::spawn(move || {
        rt.block_on(listen_relayer_register::<BridgeMessage>(
            relayer_register_rx,
        ));
    });
    relayer_register_tx
}

pub fn get_client_regiser() -> Sender<LaunchInfo<BridgeMessage>> {
    let rt = get_runtime();
    let (all_clients_tx, all_clients_rx): (
        Sender<LaunchInfo<BridgeMessage>>,
        Receiver<LaunchInfo<BridgeMessage>>,
    ) = mpsc::channel(32);
    thread::spawn(move || {
        rt.block_on(listen_clients_register(all_clients_rx));
    });
    all_clients_tx
}

async fn register_all(
    client_register_info: LaunchInfo<BridgeMessage>,
    client_register: Sender<LaunchInfo<BridgeMessage>>,
    register_info: RegisterInfo,
    relayer_register: Sender<RegisterInfo>,
    custom_task_register_info: CustomTaskInfo,
    custom_task_register: Sender<CustomTaskInfo>,
) {
    client_register.send(client_register_info).await;
    thread::sleep(Duration::from_secs(1));
    relayer_register.send(register_info).await;
    custom_task_register.send(custom_task_register_info).await;
}

fn build_register_info(name: &str, group: &str, addr: &str) -> RegisterInfo {
    RegisterInfo {
        addr: Box::new(addr.to_string()),
        name: Box::new(name.to_string()),
        group: Box::new(group.to_string()),
    }
}

fn build_client_register_info(
    name: &str,
    group: &str,
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

pub struct CustomTaskInfo{
    pub receiver:Receiver<BridgeMessage>,
    pub pool:Arc<Mutex<ThreadPool>>,
}

pub async fn listen_custom_tasks(mut receiver: Receiver<CustomTaskInfo>) -> Result<(), Box<dyn Error>>
where
{
    tokio::spawn(async move {
        while let Some(mut custom_task) = receiver.recv().await {
            tokio::spawn(async move {
                launch_custom_task(custom_task)
                .await;
            });
        }
    })
    .await;
    Ok(())
}

async fn launch_custom_task(mut task:CustomTaskInfo){
   while let Some(message)=task.receiver.recv().await{
       let mutex_pool=task.pool.lock().unwrap();
        receive_msg(message,mutex_pool);
   }
}