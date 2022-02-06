use std::io::{self, BufRead};

use custom::{get_custom, register_node};
use frame_common::get_runtime;

use tokio::runtime::Runtime;

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
    Shutdown,
}

pub fn launch_service() {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    let rt = get_runtime();

    let (mut machine, relayer) = get_custom().unwrap();

    let mut flag: bool = false;
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
                if let Err(error) = register_node(&rt, &name, &group, &addr, &mut machine, &relayer)
                {
                    println!("add client failed,error={}", error);
                }
            }
            Command::SendMsg { from, to, content } => {
                if let Err(error) = &machine.send_message(&rt, from.clone(), to.clone(), content) {
                    println!("send msg failed,error={}", error);
                }
            }
            Command::Shutdown => {
                flag = true;
                break;
            }
        }
    }

    if (flag) {
        drop(relayer);
        drop(machine);
        println!("do other customize things like drop or other tasks");
    }
}

fn parse_command(input: &str) -> Result<Command, &str> {
    match input.trim() {
        "Shutdown" => {
            return Ok(Command::Shutdown);
        }
        _ => (),
    }
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
