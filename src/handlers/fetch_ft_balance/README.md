
This lambda use Alchemy API to call `getTokenBalances` to get the FT token balance for an address.

## Request

- `chain_id`: Id of the chain to use (1, 11155111, etc.)
- `address`: Address to get the balance

Body request
```json
{
    "contract_addresses": [
		"0xdac17f958d2ee523a2206206994597c13d831ec7", 
		"0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
	]
}
```


## Response


```json
{
    "data": [
        {
            "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "name": "Tether USDt",
            "symbol": "USDT",
            "balance": "241326225647",
            "logo": "https://static.alchemyapi.io/images/assets/825.png",
            "decimals": "6"
        },
        {
            "contract_address": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "name": "USD Coin",
            "symbol": "USDC",
            "balance": "5462029835",
            "logo": "https://static.alchemyapi.io/images/assets/3408.png",
            "decimals": "6"
        }
    ],
    "errors": []
}
```

- `contract_address`: Contract address for the balance
- `name`: Name of the FT token for the chain
- `symbol`: Symbol of the FT token for the chain
- `balance`: Amount of FT tokens the address has
- `logo`: Logo of the contract.
- `decimals`: Decimal point to display the balance