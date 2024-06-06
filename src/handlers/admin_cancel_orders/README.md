# How To Call `admin_cancel_orders` lambda

In AWS console, Lambdas, find this function.  
Go to `Test` tab and in the "Event JSON" enter this:

```json
{
  "order_ids": [
    "802472d1-b0b1-40f2-8320-724f6bab0fc5",
    "802472d1-b0b1-40f2-8320-724f6bab0fc5",
    "802472d1-b0b1-40f2-8320-724f6bab0fc5"
  ]
}
```

Example response:

```json
{
    "data": [],
    "errors": [
        "802472d1-b0b1-40f2-8320-724f6bab0fc5",
        "802472d1-b0b1-40f2-8320-724f6bab0fc5",
        "802472d1-b0b1-40f2-8320-724f6bab0fc5"
    ]
}
```
