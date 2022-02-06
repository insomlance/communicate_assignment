
use super::*;

use frame_common::{data::BridgeMessage, get_rsa, sign};

use crate::RouteTable;

#[test]
fn it_works() {
    let result = 2 + 2;
    assert_eq!(result, 4);
}

#[test]
fn test_transfer() {
    let route_table: RouteTable<BridgeMessage> = Arc::new(Mutex::new(HashMap::new()));
    let pub_keys: PubKeyTable = Arc::new(Mutex::new(HashMap::new()));
    let (send_tx, send_rx): (BcMsgSender<BridgeMessage>, BcMsgReceiver<BridgeMessage>) =
        broadcast::channel(16);

    let rt = get_runtime();
    let (pr, pu) = get_rsa().unwrap();
    let a1 = "a1";
    let sig = sign("a1a", &pr).unwrap();
    let bmsg = BridgeMessage {
        from_name: Box::new(a1.to_string()),
        from_group: Box::new("a".to_string()),
        to_name: Box::new(a1.to_string()),
        to_group: Box::new("a".to_string()),
        message: Box::new("erwrew hihi".to_string()),
        error_msg: None,
        sig: Some(sig),
    };

    route_table
        .lock()
        .unwrap()
        .insert(bmsg.get_source_id(), send_tx);

    pub_keys.lock().unwrap().insert(bmsg.get_source_id(), pu);

    assert_eq!(true, transfer_msg(route_table, pub_keys, bmsg).is_ok());
}
