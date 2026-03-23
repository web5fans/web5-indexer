use dotenvy::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_base_url: String,
    pub ckb_node: String,
    pub ckb_network: String,
    pub listen_port: u64,
    pub log_level: String,
    pub worker_num: u64,
    pub start_height: u64,
    pub code_hash: String,
    pub vote_code_hash: String,
    pub dao_code_hash: String,
    pub app_mode: Vec<String>,
    pub relay_url: Option<String>,
    pub bearer_auth: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenv().ok();
        let log_level = env::var("LOG_LEVEL").unwrap_or("info".to_string());
        Self {
            data_base_url: env::var("DATABASE_URL")
                .unwrap_or("postgres://pg:password@127.0.0.1:5433/postgres".into()),
            ckb_node: env::var("CKB_NODE").unwrap_or("https://testnet.ckb.dev".into()),
            ckb_network: env::var("CKB_NETWORK").unwrap_or("ckb_testnet".into()),
            listen_port: env_int("LISTEN_PORT").unwrap_or(9533),
            log_level,
            worker_num: env_int("WORKER_NUM").unwrap_or(2),
            start_height: env_int("START_HEIGHT").unwrap_or(17_993_051),
            code_hash: env::var("CODE_HASH").unwrap_or(
                "510150477b10d6ab551a509b71265f3164e9fd4137fcb5a4322f49f03092c7c5".into(),
            ),
            vote_code_hash: env::var("VOTE_CODE_HASH").unwrap_or(
                "b140de2d7d1536cfdcb82da7520475edce5785dff90edae9073c1143d88f50c5".into(),
            ),
            dao_code_hash: env::var("DAO_CODE_HASH").unwrap_or(
                "82d76d1b75fe2fd9a27dfbaa65a039221a380d76c926f378d3f81cf3e7e13f2e".into(),
            ),
            app_mode: match env::var("APP_MODE") {
                Ok(value) => value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                Err(_) => {
                    vec!["did".to_string()]
                }
            },
            relay_url: env::var("RELAY_URL").ok(),
            bearer_auth: env::var("BEARER_AUTH").ok(),
        }
    }
}

pub fn env_int(name: &str) -> Option<u64> {
    match env::var(name) {
        Ok(str) => match str.parse::<u64>() {
            Ok(int) => Some(int),
            _ => None,
        },
        _ => None,
    }
}
