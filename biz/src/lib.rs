use std::{
    collections::HashMap,
    io::{self, BufRead},
    sync::{Arc, Mutex},
};

use client::LaunchInfo;
use common::{
    data::{BridgeMessage, Router},
    get_rsa, get_runtime, sign,
};
use custom::{
    base::{
        machine::{get_client_regiser, register_custom_tasks, CustomTaskInfo},
        node::register_node,
    },
    get_relayer, send_msg,
};

use relayer::Relayer;
use rsa::RsaPrivateKey;
use threadpool::{Builder, ThreadPool};
use tokio::{runtime::Runtime, sync::mpsc::Sender};

mod tests;

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

pub fn launch_service() {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    // if relayer can't launch,panic
    let relayer = get_relayer().unwrap();
    let custom_task_register = register_custom_tasks();
    let client_register = get_client_regiser();
    let mut input_map: HashMap<String, Sender<BridgeMessage>> = HashMap::new();
    // name->group
    let mut group_map: HashMap<String, String> = HashMap::new();
    let mut priv_keys: HashMap<String, RsaPrivateKey> = HashMap::new();

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
        if let Err(error) = handle.read_line(&mut input) {
            println!("read error {}", error.to_string());
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
                if let Err(error) = add_client(
                    &rt,
                    name,
                    group,
                    addr,
                    &mut group_map,
                    pool.clone(),
                    custom_task_register.clone(),
                    &relayer,
                    client_register.clone(),
                    &mut input_map,
                    &mut priv_keys,
                ) {
                    println!("add client failed,error={}", error);
                };
            }
            Command::SendMsg { from, to, content } => {
                if let Err(error) = send_message(
                    &rt,
                    from,
                    to,
                    content,
                    &mut input_map,
                    &mut group_map,
                    &mut priv_keys,
                ) {
                    println!("send msg failed,error={}", error);
                }
            }
        }
    }
}

fn add_client(
    rt: &Runtime,
    name: Box<String>,
    group: Box<String>,
    addr: Box<String>,
    group_map: &mut HashMap<String, String>,
    pool: Arc<Mutex<ThreadPool>>,
    custom_task_register: Sender<CustomTaskInfo>,
    relayer: &Relayer<BridgeMessage>,
    client_register: Sender<LaunchInfo<BridgeMessage>>,
    input_map: &mut HashMap<String, Sender<BridgeMessage>>,
    priv_keys: &mut HashMap<String, RsaPrivateKey>,
) -> Result<(), String> {
    check_fresh_client(&group_map, &name)?;
    let (pri_key, pub_key) = get_rsa()?;

    register_node(
        rt,
        &name,
        &group,
        &addr,
        client_register.clone(),
        relayer,
        custom_task_register.clone(),
        pool,
        pub_key,
    )
    .map(|input| input_map.insert(*name.clone(), input))?;

    priv_keys.insert(*name.clone(), pri_key);
    group_map.insert(*name.clone(), *group);

    Ok(())
}

fn send_message(
    rt: &Runtime,
    from: Box<String>,
    to: Box<String>,
    content: Box<String>,
    input_map: &HashMap<String, Sender<BridgeMessage>>,
    group_map: &HashMap<String, String>,
    priv_keys: &HashMap<String, RsaPrivateKey>,
) -> Result<(), String> {
    let name = from.clone().to_string();
    let to_name = to.clone().to_string();
    let sender = input_map.get(&name).ok_or("sender do not exist or init!")?;
    let from_group = group_map
        .get(&name)
        .ok_or("sender group do not exist or init!")?;
    let to_group = group_map
        .get(&to_name)
        .ok_or("receiver do not exist or init!")?;
    let priv_key = priv_keys.get(&name).ok_or("private key do not exist!")?;

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
