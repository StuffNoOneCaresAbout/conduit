use crate::{utils, Error, Result};
use std::convert::TryFrom;

pub const COUNTER: &str = "c";

pub struct Globals {
    pub(super) globals: sled::Tree,
    keypair: ruma::signatures::Ed25519KeyPair,
    reqwest_client: reqwest::Client,
    server_name: String,
    registration_disabled: bool,
    max_request_size: u32,
}

impl Globals {
    pub fn load(globals: sled::Tree, config: &rocket::Config) -> Result<Self> {
        let keypair = ruma::signatures::Ed25519KeyPair::new(
            &*globals
                .update_and_fetch("keypair", utils::generate_keypair)?
                .expect("utils::generate_keypair always returns Some"),
            "key1".to_owned(),
        )
        .map_err(|_| Error::bad_database("Private or public keys are invalid."))?;

        Ok(Self {
            globals,
            keypair,
            reqwest_client: reqwest::Client::new(),
            server_name: config
                .get_str("server_name")
                .unwrap_or("localhost")
                .to_owned(),
            registration_disabled: config.get_bool("registration_disabled").unwrap_or(false),
            max_request_size: config
                .get_int("max_request_size")
                .map(|x| u32::try_from(x).expect("invalid max_request_size"))
                .unwrap_or(20 * 1024 * 1024 /* 20 MiB */),
        })
    }

    /// Returns this server's keypair.
    pub fn keypair(&self) -> &ruma::signatures::Ed25519KeyPair {
        &self.keypair
    }

    /// Returns a reqwest client which can be used to send requests.
    pub fn reqwest_client(&self) -> &reqwest::Client {
        &self.reqwest_client
    }

    pub fn next_count(&self) -> Result<u64> {
        Ok(utils::u64_from_bytes(
            &self
                .globals
                .update_and_fetch(COUNTER, utils::increment)?
                .expect("utils::increment will always put in a value"),
        )
        .map_err(|_| Error::bad_database("Count has invalid bytes."))?)
    }

    pub fn current_count(&self) -> Result<u64> {
        self.globals.get(COUNTER)?.map_or(Ok(0_u64), |bytes| {
            Ok(utils::u64_from_bytes(&bytes)
                .map_err(|_| Error::bad_database("Count has invalid bytes."))?)
        })
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    pub fn registration_disabled(&self) -> bool {
        self.registration_disabled
    }
    pub fn max_request_size(&self) -> u32 { self.max_request_size }
}
