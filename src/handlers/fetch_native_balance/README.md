# Fetch Native Balance

This lambda use Alchemy API to call `eth_getBalance` to get the native token balance for an address.

## Request
    
- `chain_id`: Id of the chain to use (1, 11155111, etc.)
- `address`: Address to get the balance
- `client_id`: Internal client_id to validate address permission


## Response


```json
{
  "name": "Sepolia Ether",
  "symbol": "ETH",
  "chain_id": 11155111,
  "balance": "530000000000000000"
}
```

- `name`: Name of the native token for the chain
- `symbol`: Symbol of the native token for the chain
- `chain_id`: Id of the chain
- `balance`: Amount of native tokens the address has in wei

