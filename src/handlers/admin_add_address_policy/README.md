# How To Call `admin_add_address_policy` lambda

In AWS console, Lambdas, find this function.  
Go to `Test` tab and in the "Event JSON" enter this:

```json
{
  "address": "0x1c965d1241d0040a3fc2a030baeeefb35c155a4e",
  "chain_id": 80002,
  "client_id": "2pqul6vogr6e5mk5lbl4rh94om",
  "policy": "test-policy"
}
```
