use frame_client::LaunchInfo;
use frame_common::{get_rsa, data::BridgeMessage};
use frame_relayer::RegisterInfo;
use rsa::{RsaPrivateKey, RsaPublicKey};
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Node{
    pub input:Option<Sender<BridgeMessage>>,
    addr: Box<String>,
    name:Box<String>,
    group:Box<String>,
    private_key:RsaPrivateKey, 
    public_key:RsaPublicKey,
}

impl Node{
    pub fn new(addr:&str,name:&str,group:&str)->Result<Node,String>{
        let (pri_key, pub_key) = get_rsa()?;
        let node=Node{
            addr: Box::new(addr.to_string()),
            name: Box::new(name.to_string()),
            group: Box::new(group.to_string()),
            private_key: pri_key,
            public_key: pub_key,
            input:None,
        };
        Ok(node)
    }

    pub fn get_input(&self)->Option<&Sender<BridgeMessage>>{
        self.input.as_ref()
    }

    pub fn get_name(&self) -> &String {
        self.name.as_ref()
    }

    pub fn get_group(&self) -> &String {
        self.group.as_ref()
    }

    pub fn get_source_id(&self) -> Box<String> {
        let ans: String = self.name.to_string() + &(self.group.to_string());
        Box::new(ans)
    }

    pub fn get_private_key(&self)->&RsaPrivateKey{
        &self.private_key
    }

    pub fn get_public_key(&self)->RsaPublicKey{
        self.public_key.clone()
    }

    pub fn build_relayer_register_info(&self) -> RegisterInfo {
        RegisterInfo {
            addr: self.addr.clone(),
            name: self.name.clone(),
            group: self.group.clone(),
        }
    }

    // pub fn build_relayer_register_info(name:&str,group:&str,addr:&str) -> RegisterInfo {
    //     RegisterInfo {
    //         addr: Box::new(addr.to_string()),
    //         name: Box::new(name.to_string()),
    //         group: Box::new(group.to_string()),
    //     }
    // }

    pub fn build_machine_register_info(
        &self,
        biz_input: Receiver<BridgeMessage>,
        biz_output: Sender<BridgeMessage>,
    ) -> LaunchInfo<BridgeMessage> {
        LaunchInfo {
            addr: self.addr.clone(),
            name: self.name.clone(),
            input: Box::new(biz_input),
            output: Box::new(biz_output),
        }
    }
}