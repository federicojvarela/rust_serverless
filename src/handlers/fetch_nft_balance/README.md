
This lambda use Alchemy API to call `getNFTs` to get the NFT token balance for an address.

## Request

- `chain_id`: Id of the chain to use (1, 11155111, etc.)
- `address`: Address to get the balance

Body request
```json
{
  "contract_addresses": [
    "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
  ],
  "pagination": {
    "page_size": 10,
    "page_key": null
  }
}
```


## Response


```json
{
  "tokens": [
    {
      "contract_address": "0x5a2ded25b460759c7149d9f7b81e7eae4affb2a2",
      "name": "TestNFT",
      "symbol": "DSRV",
      "balance": "1",
      "metadata": {
        "name": "Badge #2",
        "description": "A concise Hardhat tutorial Badge NFT with on-chain SVG images like look.",
        "image": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHByZXNlcnZlQXNwZWN0UmF0aW89InhNaW5ZTWluIG1lZXQiIHZpZXdCb3g9IjAgMCAzNTAgMzUwIj48c3R5bGU+LmJhc2UgeyBmaWxsOiB3aGl0ZTsgZm9udC1mYW1pbHk6IHNlcmlmOyBmb250LXNpemU6IDE0cHg7IH08L3N0eWxlPjxyZWN0IHdpZHRoPSIxMDAlIiBoZWlnaHQ9IjEwMCUiIGZpbGw9ImJsYWNrIiAvPjx0ZXh0IHg9IjEwIiB5PSIyMCIgY2xhc3M9ImJhc2UiPjI8L3RleHQ+PC9zdmc+",
        "attributes": []
      }
    }
  ],
  "pagination": {
    "page_size": 100,
    "page_key": null
  }
}
```

- `contract_address`: Contract address for the balance
- `name`: Name of the NFT contract
- `symbol`: Symbol of the NFT contract
- `balance`: Balance of NFT token
- `metadata.name`: Name of the NFT
- `metadata.description`: A brief description of the NFT
- `metadata.image`: NFT image
- `metadata.attributes`: NFT attributes
- `pagination.page_size`: Amount of results returned 
- `pagination.page_key`: Key for querying remaining NFTs 