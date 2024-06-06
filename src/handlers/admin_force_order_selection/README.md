# How To Call `admin_force_order_selection` lambda

In AWS console, Lambdas, find this function.  
Go to `Test` tab and in the "Event JSON" enter this:

```json
{
  "order_id": "8ea236a6-8e0a-41fb-85a5-9f87a790db21"
}
```
You can use any order_id you like, it is not validated at this point.