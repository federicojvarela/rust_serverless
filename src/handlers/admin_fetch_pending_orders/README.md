# How To Call `admin_fetch_pending_orders` lambda

In AWS console, Lambdas, find this function.  
Go to `Test` tab and in the "Event JSON" enter this:

```json
{
  "key_id": "802472d1-b0b1-40f2-8320-724f6bab0fc5",
  "chain_id": 80002
}
```
You can use any key_id you like, it is not validated at this point.