
use std::{
    thread::{self},
    time::Duration,
};

use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    runtime::Runtime,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::{listen_clients_register, LaunchInfo};

fn get_runtime() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

async fn get_stream(
    addr: &str,
    launch_info: LaunchInfo<String>,
    sender: Sender<LaunchInfo<String>>,
) -> TcpStream {
    sender.send(launch_info).await;

    thread::sleep(Duration::from_secs(1));
    TcpStream::connect(addr).await.unwrap()
}

#[test]
fn test_client() {
    let rt = get_runtime();
    let rt1 = get_runtime();

    let (all_clients_tx, all_clients_rx): (
        Sender<LaunchInfo<String>>,
        Receiver<LaunchInfo<String>>,
    ) = mpsc::channel(32);
    thread::spawn(move || {
        rt1.block_on(listen_clients_register(all_clients_rx));
    });

    let (input_tx1, input_rx1): (Sender<String>, Receiver<String>) = mpsc::channel(32);
    let (output_tx1, output_rx1): (Sender<String>, Receiver<String>) = mpsc::channel(32);
    let addr = "127.0.0.1:8787";
    let launch_info = LaunchInfo {
        addr: Box::new(addr.to_string()),
        input: Box::new(input_rx1),
        output: Box::new(output_tx1),
        name: Box::new("A1".to_string()),
    };
    let all_clients_tx_1 = all_clients_tx.clone();
    let mut r1 = rt.block_on(async { get_stream(addr, launch_info, all_clients_tx_1).await });

    let (input_tx2, input_rx2): (Sender<String>, Receiver<String>) = mpsc::channel(32);
    let (output_tx2, _output_rx2): (Sender<String>, Receiver<String>) = mpsc::channel(32);
    let addr = "127.0.0.1:9787";
    let launch_info = LaunchInfo {
        addr: Box::new(addr.to_string()),
        input: Box::new(input_rx2),
        output: Box::new(output_tx2),
        name: Box::new("B1".to_string()),
    };
    let all_clients_tx_2 = all_clients_tx.clone();
    let mut r2 = rt.block_on(async { get_stream(addr, launch_info, all_clients_tx_2).await });

    rt.block_on(async {
        r1.write_all("\"response to 1\"".as_bytes()).await;
    });

    rt.block_on(async {
        r2.write_all("\"response to 2\"".as_bytes()).await;
    });

    rt.block_on(async {
        input_tx1.send("message from 1".to_string()).await;
        input_tx2.send("message from 2".to_string()).await;
    });
    loop {}
}

#[test]
fn test_bind() {
    let rt = get_runtime();
    rt.block_on(async {
        TcpListener::bind("127.0.0.1:8979").await;
        println!("bind1");
    });
    thread::sleep(Duration::from_secs(2));
    rt.block_on(async {
        TcpListener::bind("127.0.0.1:8979").await;
        println!("bind2");
    });
}
