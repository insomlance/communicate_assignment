
use std::{thread, time::Duration};

use custom::machine::Machine;

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

    let (mut machine,relayer)=get_custom().unwrap();

    register_node(&rt, "A1", "A", "127.0.0.1:8787", &mut machine, &relayer);
    register_node(&rt, "B1", "B", "127.0.0.1:9787", &mut machine, &relayer);

    // for i in 0..100 {
    //     do_test(
    //         &rt,
    //         &machine,
    //         i,
    //     );
    // }

    &machine.send_message(&rt, Box::new("A1".to_string()), Box::new("B1".to_string()), Box::new("this is a test".to_string()));

    loop {}
}

fn do_test(
    rt: &Runtime,
    machine:&Machine,
    seq: u8,
) {
    machine.send_message(&rt, Box::new("A1".to_string()), Box::new("B1".to_string()), Box::new("this is a test".to_string()+ &seq.to_string()));
}
