use std::iter::repeat;

use crypto::{digest::Digest, sha2::Sha256};
use log::error;
use rand::rngs::OsRng;
use rsa::{Hash, PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey};
use serde::{de::DeserializeOwned, Serialize};
use tokio::runtime::Runtime;

pub mod data;

pub fn get_runtime() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

pub fn get_rsa() -> Result<(RsaPrivateKey, RsaPublicKey), String> {
    let mut rng = OsRng;
    let bits = 2048;

    let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let public_key = RsaPublicKey::from(&private_key);
    Ok((private_key, public_key))
}

pub fn sign(data: &str, private_key: &RsaPrivateKey) -> Result<Vec<u8>, String> {
    let buf = get_hash(data);
    let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));

    private_key
        .sign(padding, &buf)
        .map_err(|_| "sign failed".to_string())
}

pub fn verify(data: &str, public_key: &RsaPublicKey, sig: &Vec<u8>) -> Result<(), String> {
    let buf = get_hash(data);
    let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));

    public_key
        .verify(padding, &buf, &sig)
        .map_err(|_| "verify failed".to_string())
}

pub fn get_hash(data: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.input_str(data);
    let mut buf: Vec<u8> = repeat(0).take((hasher.output_bits() + 7) / 8).collect();
    hasher.result(&mut buf);
    buf
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
    use crate::{get_rsa, sign, verify};

    #[test]
    fn test_rsa() {
        let (pr, pu) = get_rsa().unwrap();
        let data = "it's a rsa test";
        let sig = sign(data, &pr).unwrap();
        assert_eq!(true, verify(data, &pu, &sig).is_ok());
    }
}
