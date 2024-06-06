# How To Call `admin_update_address_nonce`

In AWS console, Lambdas, find this function.
Go to `Test` tab and in the "Event JSON" enter this:

```json
{
  "address": "<address>",
  "chain_id": <chain_id>
}
```

