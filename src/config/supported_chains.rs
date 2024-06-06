use ethers::types::Chain;

// 1        -> Eth Mainnet
// 11155111 -> Eth Sepolia Testnet
// 80002     -> Polygon Amoy Testnet
// 137      -> Polygon Mainnet
// 1337     -> Ganache
const SUPPORTED_CHAIN_IDS: [u64; 5] = [1, 11155111, 80002, 137, 1337];

pub trait SupportedChain {
    fn is_supported(&self) -> bool;
}

impl SupportedChain for Chain {
    fn is_supported(&self) -> bool {
        let chain_id = *self as u64;
        SUPPORTED_CHAIN_IDS.contains(&chain_id)
    }
}

impl SupportedChain for u64 {
    fn is_supported(&self) -> bool {
        SUPPORTED_CHAIN_IDS.contains(self)
    }
}
