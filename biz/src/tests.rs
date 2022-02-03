
use std::{thread, time::Duration};

use super::*;
use crate::parse_command;

#[test]
fn test_input() {
    let mut res = parse_command("AddClient{A1;A;127.0.0.1:8787}");
    assert_eq!(true, res.is_ok());
    res = parse_command("AddjClient{A1;A;127.0.0.1:8787}");
    assert_eq!(true, res.is_err());
    res = parse_command("AddClient{A1,A;127.0.0.1:8787}");
    assert_eq!(true, res.is_err());
    res = parse_command("AddClient {A1;A;127.0.0.1:8787}");
    assert_eq!(true, res.is_err());
    res = parse_command("SendMsg{A1;A2;this is A1, to A group}");
    assert_eq!(true, res.is_ok());
}

#[test]
fn test_func() {
    let rt = get_runtime();
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
            .num_threads(4)
            .thread_name(String::from("threadpool"))
            .build(),
    ));

    add_client(
        &rt,
        Box::new("A1".to_string()),
        Box::new("A".to_string()),
        Box::new("127.0.0.1:8787".to_string()),
        &mut group_map,
        pool.clone(),
        custom_task_register.clone(),
        &relayer,
        client_register.clone(),
        &mut input_map,
        &mut priv_keys,
    );
    add_client(
        &rt,
        Box::new("B1".to_string()),
        Box::new("B".to_string()),
        Box::new("127.0.0.1:9787".to_string()),
        &mut group_map,
        pool.clone(),
        custom_task_register.clone(),
        &relayer,
        client_register.clone(),
        &mut input_map,
        &mut priv_keys,
    );

    for i in 0..100 {
        do_test(
            &rt,
            &group_map,
            client_register.clone(),
            &input_map,
            &priv_keys,
            i,
        );
    }

    send_message(
        &rt,
        Box::new("A1".to_string()),
        Box::new("A1".to_string()),
        Box::new("this is a test".to_string()),
        &input_map,
        &group_map,
        &priv_keys,
    );

    loop {}
}

fn do_test(
    rt: &Runtime,
    group_map: &HashMap<String, String>,
    // pool: Arc<Mutex<ThreadPool>>,
    // custom_task_register: Sender<CustomTaskInfo>,
    // relayer: &Relayer<BridgeMessage>,
    //client_register: Sender<LaunchInfo<BridgeMessage>>,
    input_map: &HashMap<String, Sender<BridgeMessage>>,
    priv_keys: &HashMap<String, RsaPrivateKey>,
    seq: u8,
) {
    send_message(
        &rt,
        Box::new("A1".to_string()),
        Box::new("B1".to_string()),
        Box::new("this is a test ".to_string() + &seq.to_string()),
        input_map,
        group_map,
        priv_keys,
    );
}
