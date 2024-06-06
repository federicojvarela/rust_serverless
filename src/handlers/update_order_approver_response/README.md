# `update_order_approver_response` lambda

In the `process-approvers-response` workflow we need to update the order's list of approvals based on which approver 
triggered the workflow. For us to support multiple approvers, we need to prepare the order record which involves 
looping over the list of known approvers for this order, and mapping the approver's response to its item stored
within the order record. This lambda does that preparation. The next step in the workflow after this lambda is called
will be to update the order record in DynamoDB.

#### This lambda is intended to be called as part of a workflow.

# Input

```json
{
  "approval_status": 1,
  "approver_name": "forte_waas_txn_approver",
  "metadata": "eyJhcHByb3ZhbF9zdGF0dXMiOjEsIm9yZGVyX2lkIjoiNDQ1NTI5YmYtNDVjNS00NzgwLThiNWUtOTgxOWMxNDIxMjUwIiwic3RhdHVzX3JlYXNvbiI6IlRoaXMgaXMgYW4gYXV0by1hcHByb3ZlZCB0cmFuc2FjdGlvbiIsInRyYW5zYWN0aW9uX2hhc2giOlsxMDgsMzcsMzcsMjA5LDYwLDIzMywxNDIsMjI4LDE0MiwxNTQsNzIsMTc0LDExMSwxOTYsNDYsNTksMTQ2LDI0MSwyNDQsMTA1LDQxLDQ0LDI1MSwwLDEwNiwxNzksMTk1LDIwMCwxMzEsMTMzLDMxLDIwMl19",
  "metadata_signature": "MEUCIQCXVE7ioy23HUKcWhoZAjemgCqeR9iZJ9ApciD7syre5AIgF3ehYtx0XQOb2b+DtLDuKF+qkheojQxxL6/HcCIaFjU=",
  "order_id": "445529bf-45c5-4780-8b5e-9819c1421250",
  "status_reason": "This is an auto-approved transaction",
  "fetched": {
    "order": {
      "data": {
        "address": "0x5bc4c5da4f545ffa8865ae9655ed9a98a470f680",
        "client_id": "132d3l0888j528k7vhss40gspv",
        "key_id": "88967797-55be-46d8-ba67-21f63e18083e",
        "transaction": {
          "chain_id": 11155111,
          "data": "0x00",
          "gas": "0x55f0",
          "max_fee_per_gas": "0x1",
          "max_priority_fee_per_gas": "0x1",
          "to": "0x3efdd74dd510542ff7d7e4ac1c7039e4901f3ab1",
          "value": "0x1"
        }
      },
      ...,
      ...,
      ...,
      "policy": {
        "name": "DefaultTenantSoloApproval",
        "approvals": [
          {
            "level": "Tenant",
            "name": "forte_waas_txn_approver"
          }
        ]
      }
    }
  }
}
```

# Output

```json
{
  "policy": {
    "name": "DefaultTenantSoloApproval",
    "approvals": [
      {
        "level": "Tenant",
        "name": "forte_waas_txn_approver",
        "response": {
          "order_id": "....",
          "status_reason": "...",
          "approval_status": 1,
          "approver_name": "...",
          "metadata": "...",
          "metadata_signature": "...."
        }
      }
    ]
  }
}
```