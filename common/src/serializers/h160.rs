use ethers::types::H160;
use hex::ToHex;

pub fn h160_to_lowercase_hex_string(address: H160) -> String {
    format!("0x{}", address.encode_hex::<String>().to_lowercase())
}
