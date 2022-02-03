use common::data::BridgeMessage;
use rsa::RsaPublicKey;
use std::{
    error::Error,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use threadpool::ThreadPool;
use tokio::sync::mpsc::{Receiver, Sender};

use client::LaunchInfo;
use relayer::{RegisterInfo, Relayer};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self},
};

use crate::receive_msg;

pub mod node {
    use crate::base::*;

    use super::machine::CustomTaskInfo;

    pub fn register_node(
        rt: &Runtime,
        name: &str,
        group: &str,
        addr: &str,
        client_register: Sender<LaunchInfo<BridgeMessage>>,
        relayer: &Relayer<BridgeMessage>,
        custom_task_register: Sender<CustomTaskInfo>,
        pool: Arc<Mutex<ThreadPool>>,
        pub_key: RsaPublicKey,
    ) -> Result<Sender<BridgeMessage>, String> {
        let (input_tx, input_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) =
            mpsc::channel(32);
        let (output_tx, output_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) =
            mpsc::channel(32);

        let register_info = build_register_info(name, group, addr);
        let client_register_info = build_client_register_info(name, addr, input_rx, output_tx);
        let custom_task_register_info = CustomTaskInfo {
            receiver: output_rx,
            pool: pool,
        };
        rt.block_on(async {
            register_all(
                client_register_info,
                client_register,
                register_info,
                relayer,
                custom_task_register_info,
                custom_task_register,
                pub_key,
            )
            .await
        })?;

        Ok(input_tx)
    }

    async fn register_all(
        client_register_info: LaunchInfo<BridgeMessage>,
        client_register: Sender<LaunchInfo<BridgeMessage>>,
        register_info: RegisterInfo,
        relayer: &Relayer<BridgeMessage>,
        custom_task_register_info: CustomTaskInfo,
        custom_task_register: Sender<CustomTaskInfo>,
        pub_key: RsaPublicKey,
    ) -> Result<(), String> {
        client_register
            .send(client_register_info)
            .await
            .map_err(|err| err.to_string())?;
        thread::sleep(Duration::from_secs(1));
        relayer
            .register_node(register_info, pub_key)
            .await
            .map_err(|err| err.to_string())?;
        custom_task_register
            .send(custom_task_register_info)
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
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
}

pub mod machine {
    use client::listen_clients_register;
    use common::get_runtime;

    use super::*;
    pub struct CustomTaskInfo {
        pub receiver: Receiver<BridgeMessage>,
        pub pool: Arc<Mutex<ThreadPool>>,
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

    pub fn register_custom_tasks() -> Sender<CustomTaskInfo> {
        let rt = get_runtime();
        let (tx, rx) = mpsc::channel(8);
        thread::spawn(move || {
            rt.block_on(listen_custom_tasks(rx));
        });
        tx
    }

    async fn listen_custom_tasks(
        mut receiver: Receiver<CustomTaskInfo>,
    ) -> Result<(), Box<dyn Error>> {
        tokio::spawn(async move {
            while let Some(custom_task) = receiver.recv().await {
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
}
