use std::{
    collections::HashMap,
    error::Error,
    io::{self, BufRead},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use client::{listen_clients_register, LaunchInfo};
use common::{data::BridgeMessage, get_runtime};
use custom::{send_msg, base::{register_custom_tasks, get_relayer_register, get_client_regiser, register_node}};
use log::{debug, error, info};
use relayer::{listen_relayer_register, RegisterInfo};
use threadpool::{Builder, ThreadPool};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self, Receiver, Sender},
};

#[derive(Debug)]
enum Command {
    AddClient {
        name: Box<String>,
        group: Box<String>,
        addr: Box<String>,
    },
    SendMsg {
        from: Box<String>,
        to: Box<String>,
        content: Box<String>,
    },
}

fn main() {
   launch_service();
}

pub fn launch_service() {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    let custom_task_register = register_custom_tasks();
    let relayer_register = get_relayer_register();
    let client_register = get_client_regiser();
    let mut input_map: HashMap<String, Sender<BridgeMessage>> = HashMap::new();
    /// name->group
    let mut group_map: HashMap<String, String> = HashMap::new();

    //custom thread pool
    let pool = Arc::new(Mutex::new(
        Builder::new()
            .num_threads(2)
            .thread_name(String::from("threadpool"))
            .build(),
    ));

    let rt = get_runtime();

    loop {
        input.clear();
        if let Err(error)=handle.read_line(&mut input){
            println!("read error {}",error.to_string());
            continue;
        }
        let result = parse_command(&input);
        match result {
            Err(error) => {
                println!("{}", error);
                continue;
            }
            Ok(_) => (),
        };
        let command = result.unwrap();
        match command {
            Command::AddClient { name, group, addr } => {
                if let Err(msg) = check_fresh_client(&group_map, &name) {
                    error!("fresh client check failed for {}", msg);
                    println!("add fresh client failed, {}", msg);
                    continue;
                };
                let clone_pool = pool.clone();
                let input = register_node(
                    &name,
                    &group,
                    &addr,
                    client_register.clone(),
                    relayer_register.clone(),
                    custom_task_register.clone(),
                    clone_pool,
                );
                input_map.insert(*name.clone(), input);
                group_map.insert(*name.clone(), *group);
            }
            Command::SendMsg { from, to, content } => {
                let name = from.clone().to_string();
                let sender = input_map.get(&name);
                let from_group = group_map.get(&name);
                if sender.is_none() || from_group.is_none(){
                    println!("sender do not exist or init!");
                    continue;
                }

                let to_name = to.clone().to_string();
                let to_group = group_map.get(&to_name);
                if to_group.is_none(){
                    println!("receiver do not exist or init!");
                    continue;
                }

                let sender=sender.unwrap();
                let from_group=from_group.unwrap();
                let to_group=to_group.unwrap();

                let bridge_message = BridgeMessage {
                    from_name: Box::new(name),
                    from_group: Box::new(from_group.to_string()),
                    to_name: Box::new(to_name),
                    to_group: Box::new(to_group.to_string()),
                    message: content,
                    error_msg: None,
                };

                send_msg(sender, &rt, bridge_message);
            }
        }
    }
}

fn check_fresh_client(map: &HashMap<String, String>, name: &str) -> Result<(), String> {
    if map.contains_key(name) {
        return Err("client already exitst".to_string());
    }
    Ok(())
}

fn parse_command(input: &str) -> Result<Command, &str> {
    let inputs: Vec<&str> = input.trim().split('{').collect();
    if inputs.len() != 2 {
        return Err("Error! input style should be 'Command{{xxx}}'");
    }

    let command_str = inputs.get(0).unwrap();
    let command: Command;
    match *command_str {
        "AddClient" => {
            let info = inputs.get(1).unwrap().replace('}', "");
            let infos: Vec<&str> = info.trim().split(';').collect();
            if infos.len() != 3 {
                return Err("Error! AddClient info should be 'name;group;address'");
            }
            command = Command::AddClient {
                name: Box::new((*infos.get(0).unwrap()).to_string()),
                group: Box::new((*infos.get(1).unwrap()).to_string()),
                addr: Box::new((*infos.get(2).unwrap()).to_string()),
            };
        }
        "SendMsg" => {
            let info = inputs.get(1).unwrap().replace('}', "");
            let infos: Vec<&str> = info.trim().split(';').collect();
            if infos.len() != 3 {
                return Err("Error! SendMsg info should be 'from;to;content'");
            }
            command = Command::SendMsg {
                from: Box::new((*infos.get(0).unwrap()).to_string()),
                to: Box::new((*infos.get(1).unwrap()).to_string()),
                content: Box::new((*infos.get(2).unwrap()).to_string()),
            };
        }
        _ => return Err("command not support"),
    }
    Ok(command)
}


