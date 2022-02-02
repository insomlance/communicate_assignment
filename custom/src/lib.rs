use std::{sync::MutexGuard, thread};

use base::{CustomTaskInfo, listen_custom_tasks};
use client::{LaunchInfo, listen_clients_register};
use common::{data::BridgeMessage, get_runtime};
use relayer::{RegisterInfo, listen_relayer_register, Relayer};
use threadpool::ThreadPool;
use tokio::{sync::mpsc::{Sender, self, Receiver}, runtime::Runtime};

pub mod base;

pub fn receive_msg(message:BridgeMessage,mutex_pool:MutexGuard<ThreadPool>){
    print!("{}: receive msg from {}: ",message.to_name,message.from_name);
    println!("{}",message.message);
    match message.to_group.as_str(){
        "A"=>mutex_pool.execute(move || message_to_a(&message.message)),
        "B"=>mutex_pool.execute(move || message_to_b(&message.message)),
        "C"=>mutex_pool.execute(move || message_to_c(&message.message)),
        _=> println!("no special task for the group"),
    }
}

pub fn send_msg(sender:&Sender<BridgeMessage>,rt:&Runtime,bridge_message:BridgeMessage){
    let mut extra_process: Option<Box<dyn Fn()>> = None;
    match bridge_message.from_group.as_str() {
        "A" => extra_process = Some(Box::new(&message_from_a)),
        "B" => extra_process = Some(Box::new(&message_from_b)),
        "C" => extra_process = Some(Box::new(&message_from_c)),
        _ => (),
    }
    send_msg_group(extra_process);
    rt.block_on(async {sender.send(bridge_message).await;})
}

fn send_msg_group(extra_process:Option<Box<dyn Fn()>>){
    if let Some(box_fn)=extra_process{
        box_fn();
    }
}

fn message_from_a(){
    println!("MsgFromA: do things for group a before send");
}

fn message_from_b(){
    println!("MsgFromB: do things for group b before send");
}

fn message_from_c(){
    println!("MsgFromC: do things for group c before send");
}

fn message_to_a(msg:&str){
    println!("MsgToA: do some task for group a");
}

fn message_to_b(msg:&str){
    println!("MsgToB: do some task for group b");
}

fn message_to_c(msg:&str){
    println!("MsgToC: do some task for group c");
}

pub fn register_custom_tasks() -> Sender<CustomTaskInfo> {
    let rt=get_runtime();
    let (tx, mut rx) = mpsc::channel(8);
    thread::spawn(move || {
        rt.block_on(listen_custom_tasks(rx));
    });
    tx
}

pub fn get_relayer() -> Result<Relayer<BridgeMessage>,String>{
    let mut relayer=Relayer::<BridgeMessage>::new();
    relayer.launch();
    if !relayer.is_ready(){
        return Err("launch relayer failed".to_string());
    }
    Ok(relayer)
}

// pub fn get_relayer_register() -> Sender<RegisterInfo> {
//     let rt = get_runtime();
//     let (relayer_register_tx, relayer_register_rx): (Sender<RegisterInfo>, Receiver<RegisterInfo>) =
//         mpsc::channel(32);
//     thread::spawn(move || {
//         rt.block_on(listen_relayer_register::<BridgeMessage>(
//             relayer_register_rx,
//         ));
//     });
//     relayer_register_tx
// }

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


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
