use std::sync::{Arc, RwLock};

use crate::config::Config;
use crate::provider::Provider;

pub mod session;

pub struct Server {
    provider: Arc<RwLock<Provider>>,
    config: Arc<RwLock<Config>>,
    port: Option<u16>,
}

impl Server {
    pub fn new(
        provider: Arc<RwLock<Provider>>,
        config: Arc<RwLock<Config>>,
        port: Option<u16>,
    ) -> Self {
        Self {
            provider,
            config,
            port,
        }
    }

    pub async fn run(self) {
        let port = self.port.unwrap_or(8080);
        println!("Server would start on port {}", port);
    }
}
