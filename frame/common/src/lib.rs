use serde::{Serialize, de::DeserializeOwned};
use log::error;
use tokio::runtime::Runtime;

pub mod data;

pub fn get_runtime() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

pub fn parse_message_list<T>(raw_msg: &str) -> Vec<T>
where
    T: Serialize + DeserializeOwned,
{
    let mut res: Vec<T> = Vec::new();
    let json_list: Vec<&str> = raw_msg.split("/*1^/").filter(|f| f.len() > 0).collect();
    json_list.into_iter().for_each(|json| {
        let ans = serde_json::from_str(json);
        if let Err(error) = &ans {
            error!("parse json error,json={},error={}", json, error);
            return;
        }
        let parsed: T = ans.unwrap();
        res.push(parsed);
    });
    res
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
