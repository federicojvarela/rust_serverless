use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct ChainNetwork {
    pub name: String,
    pub id: u64,
    pub nft_contract_address: String,
    pub ft_contract: String,
}

#[derive(Debug, Deserialize)]
pub struct Chain {
    pub name: String,
    pub chain_network: Vec<ChainNetwork>,
}

#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "select_chain")]
    pub chain: Chain,
    pub environment: String,
    pub ephemeral: bool,
    pub env_url: String,
    pub funded_address: String,
    pub funded_address_public_key: String,
    pub gas_pool_address_e2e: String,
    pub custodial_address: String,
    pub authorization_token: String,
    pub client_user_id: String,
    pub auth_url: String,
    pub timeout: u64,
    pub client_id: String,
    pub default_approver_name: String,
    pub custom_approver_name: String,

    // optional values for ephemeral env e2e testing
    pub dev_env_url: Option<String>,
    pub dev_auth_url: Option<String>,
    pub dev_authorization_token: Option<String>,
    pub dev_client_id: Option<String>,
    pub dev_funded_address: Option<String>,
}

fn select_chain() -> Chain {
    let chain_name: &str = &env::var("CHAIN_NAME").unwrap();
    match chain_name {
        "ethereum" => Chain {
            name: "Ethereum".to_owned(),
            chain_network: vec![ChainNetwork {
                name: "Sepolia".to_owned(),
                id: 11155111,
                nft_contract_address: "0x5a2ded25b460759c7149d9f7b81e7eae4affb2a2".to_owned(),
                ft_contract: "0x24d609e49655a9896259dafbb788a0a73aa14cdd".to_owned(),
            }],
        },
        "polygon" => Chain {
            name: "Polygon".to_owned(),
            chain_network: vec![ChainNetwork {
                name: "Amoy".to_owned(),
                id: 80002,
                nft_contract_address: "0x71060a0c8a3e7db744d0c8c12b5f2e2b83ba1293".to_owned(),
                ft_contract: "0xecd6b099569fe13ce818d9c9287417ca9308733b".to_owned(),
            }],
        },
        _ => {
            panic!("Chain {} is not configured for e2e tests", chain_name);
        }
    }
}

impl Config {
    pub fn get_network_by_chain_id(&self, chain_id: u64) -> ChainNetwork {
        if let Some(network) = self
            .chain
            .chain_network
            .iter()
            .find(|network| network.id == chain_id)
        {
            return network.clone();
        }
        panic!(
            "Chain {} does not have the network ID {} configured for e2e tests",
            self.chain.name, chain_id
        );
    }
}
const ENVS: [&str; 7] = [
    "dev", "staging", "qa", "loadtest", "sandbox", "prod", "local",
];

impl Config {
    pub fn load_test() -> Self {
        let env_var: &str = &env::var("ENV").unwrap_or("test".to_string());
        if ENVS.contains(&env_var) {
            dotenv::from_filename(format!("./e2e/env_files/.env.test.e2e.{env_var}")).ok();
        } else {
            dotenv::from_filename(format!("./e2e/env_files/.env.test.e2e.{env_var}.eph")).ok();
        }

        dotenv::from_filename(".env.test.local").ok();
        dotenv::from_filename(".env.test").ok();
        dotenv::from_filename(".env.local").ok();
        dotenv::from_filename(".env").ok();

        envy::from_env::<Self>().expect("Could not load configuration")
    }
}
