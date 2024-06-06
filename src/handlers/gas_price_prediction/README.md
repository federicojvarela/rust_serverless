# Predicting Gas Price from Pending Transactions

This example is to illustrate how we estimate the gas price from a blockchain node and calculate the min, max and median fees that we return to clients.

## Part 1: `maxPriorityFeePerGas`

We need to find this:

```json
{
  "maxPriorityFeePerGas": {
    "low": "?",
    "medium": "?",
    "high": "?"
  }
}
```

1. We will get the transactions from pending block (`BlockNumber::Pending`).
2. Then we get the `max_priority_fee_per_gas` for each one.
3. Then we calculate the suggested percentiles from those values:

```json
{
  "maxPriorityFeePerGas": {
    "low": "{ 25th percentile }",
    "medium": "{ 50th percentile }",
    "high": "{ 95th percentile }"
  }
}
```

## Part 2: `maxFeePerGas`

We need to find this:

```json
{
  "maxFeePerGas": {
    "low": "?",
    "medium": "?",
    "high": "?"
  }
}
```

1. Get `baseFeePerGas` from pending block
2. Add `maxPriorityFeePerGas` and `baseFeePerGas` together

```json
{
  "maxFeePerGas": {
    "low": "{ maxPriorityFeePerGas.low + baseFeePerGas }",
    "medium": "{ maxPriorityFeePerGas.medium + baseFeePerGas }",
    "high": "{ maxPriorityFeePerGas.high + baseFeePerGas }"
  }
}
```

## Part 3: Combine Parts 1 and 2 to make the Response Object

```json
{
  "chain_id": "1",
  "eip-1559": {
    "maxPriorityFeePerGas": {
      "low": "{ 25th percentile }",
      "medium": "{ 50th percentile }",
      "high": "{ 95th percentile }"
    },
    "maxFeePerGas": {
      "low": "{ maxPriorityFeePerGas.low + baseFeePerGas }",
      "medium": "{ maxPriorityFeePerGas.medium + baseFeePerGas }",
      "high": "{ maxPriorityFeePerGas.high + baseFeePerGas }"
    }
  },
  // the same as `maxFeePerGas`
  "legacy": {
    "gasPrice": {
      "low": "{ maxFeePerGas.low }",
      "medium": "{ maxFeePerGas.medium }",
      "high": "{ maxFeePerGas.high }"
    }
  }
}
```
