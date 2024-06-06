use ethers::{
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{transaction::eip2718::TypedTransaction, Address, Bytes, TransactionRequest, H256},
};
use hex::ToHex;

// Function that creates and RLP encoded transaction that sends 1 WEI
pub async fn get_transaction(
    from_address_private_key: H256,
    from_address: Address,
    to_address: Address,
    chain_id: u64,
) -> Bytes {
    let wallet = from_address_private_key
        .encode_hex::<String>()
        .parse::<LocalWallet>()
        .unwrap()
        .with_chain_id(chain_id);

    let tx: TypedTransaction = TransactionRequest::new()
        .from(from_address)
        .to(to_address)
        .value(1)
        .gas(22000)
        .gas_price(900000000)
        .chain_id(chain_id)
        .into();

    let signature = wallet.sign_transaction(&tx).await.unwrap();
    tx.rlp_signed(&signature)
}

pub async fn send_transaction(provider: &Provider<Http>, transaction: Bytes) -> H256 {
    provider
        .send_raw_transaction(transaction)
        .await
        .map(|tx| tx.tx_hash())
        .unwrap()
}
