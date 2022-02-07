use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
    thread,
};

use frame_client::{listen_clients_register, LaunchInfo};
use frame_common::{data::Router, get_runtime, sign};
use threadpool::Builder;
use tokio::sync::mpsc::{self, Receiver};

use crate::node::Node;

use super::*;

pub struct Machine {
    task_register: Sender<CustomTaskInfo>,
    client_register: Sender<LaunchInfo<BridgeMessage>>,
    pool: Arc<Mutex<ThreadPool>>,
    nodes: HashMap<String, Node>,
}

impl Machine {
    pub fn new() -> Machine {
        let pool = Arc::new(Mutex::new(
            Builder::new()
                .num_threads(2)
                .thread_name(String::from("threadpool"))
                .build(),
        ));

        Machine {
            task_register: Machine::register_custom_tasks(),
            client_register: Machine::get_client_regiser(),
            pool,
            nodes: HashMap::new(),
        }
    }

    pub fn send_message(
        &self,
        rt: &Runtime,
        from: Box<String>,
        to: Box<String>,
        content: Box<String>,
    ) -> Result<(), String> {
        let name = from.clone().to_string();
        let to_name = to.clone().to_string();
        let sender = self
            .nodes
            .get(&name)
            .and_then(|node| node.get_input())
            .ok_or("sender do not exist or init!")?;
        let from_group = self
            .nodes
            .get(&name)
            .map(|node| node.get_group())
            .ok_or("sender group do not exist or init!")?;
        let priv_key = self
            .nodes
            .get(&name)
            .map(|node| node.get_private_key())
            .ok_or("private key do not exist!")?;

        let to_group = self
            .nodes
            .get(&to_name)
            .map(|node| node.get_group())
            .ok_or("receiver do not exist or init!")?;

        let mut bridge_message = BridgeMessage {
            from_name: Box::new(name),
            from_group: Box::new(from_group.to_string()),
            to_name: Box::new(to_name),
            to_group: Box::new(to_group.to_string()),
            message: content,
            error_msg: None,
            sig: None,
        };

        let sig = sign(&bridge_message.get_source_id(), priv_key)?;
        bridge_message.sig = Some(sig);

        send_msg(sender, &rt, bridge_message);
        Ok(())
    }

    pub async fn register_node(
        &mut self,
        relayer: &Relayer<BridgeMessage>,
        name: &str,
        group: &str,
        addr: &str,
    ) -> Result<(), String> {
        let mut node = Node::new(addr, name, group)?;

        if self.nodes.contains_key(node.get_name()) {
            return Err("node already exitst".to_string());
        }

        let (input_tx, input_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) =
            mpsc::channel(32);
        let (output_tx, output_rx): (Sender<BridgeMessage>, Receiver<BridgeMessage>) =
            mpsc::channel(32);

        let machine_register_info = node.build_machine_register_info(input_rx, output_tx);
        let task_register_info = CustomTaskInfo {
            receiver: output_rx,
            pool: self.pool.clone(),
        };

        self.client_register
            .clone()
            .send(machine_register_info)
            .await
            .map_err(|err| err.to_string())?;

        self.task_register
            .clone()
            .send(task_register_info)
            .await
            .map_err(|err| err.to_string())?;

        thread::sleep(Duration::from_secs(1));
        let register_info = node.build_relayer_register_info();
        relayer
            .register_node(register_info, node.get_public_key())
            .await?;

        node.input = Some(input_tx);
        self.nodes.insert(node.get_name().to_string(), node);

        Ok(())
    }

    fn register_custom_tasks() -> Sender<CustomTaskInfo> {
        let rt = get_runtime();
        let (tx, rx) = mpsc::channel(8);
        thread::spawn(move || {
            rt.block_on(Machine::listen_custom_tasks(rx));
        });
        tx
    }

    fn get_client_regiser() -> Sender<LaunchInfo<BridgeMessage>> {
        let (all_clients_tx, all_clients_rx): (
            Sender<LaunchInfo<BridgeMessage>>,
            Receiver<LaunchInfo<BridgeMessage>>,
        ) = mpsc::channel(32);
        let rt = get_runtime();
        thread::spawn(move || {
            rt.block_on(listen_clients_register(all_clients_rx));
        });
        all_clients_tx
    }

    async fn listen_custom_tasks(
        mut receiver: Receiver<CustomTaskInfo>,
    ) -> Result<(), Box<dyn Error>> {
        tokio::spawn(async move {
            while let Some(custom_task) = receiver.recv().await {
                tokio::spawn(async move {
                    Machine::launch_custom_task(custom_task).await;
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
