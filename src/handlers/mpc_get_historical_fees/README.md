# Getting Historical Fees From Node Values

This example is to illustrate how we take the historical fees from a blockchain node and calculate the min, max and median fees that we return to clients.
The context for this example can be found [here](https://www.notion.so/Historical-Fees-Provider-Implementation-117844e9b3724218a036de7687a19dcf).

## Sample Input

Here is the input we will use in our example (simplified):

```json
{
  "id": 1,
  "result": {
    // Each reward array is for one block,
    // with one value for each percentile
    // [0%, 50%, 100%]
    "reward": [
      [2, 4, 9],
      [0, 7, 8],
      [3, 5, 14]
    ],
    "baseFeePerGas": [96, 38, 50]
  }
}
```

## Part 1: `maxPriorityFeePerGas`

We need to find this:

```json
{
  "maxPriorityFeePerGas": {
    "min": "?",
    "median": "?",
    "max": "?"
  }
}
```

The input we will use is the `reward` portion of our original json:

```json
{
  "reward": [
    [2, 4, 9],
    [0, 7, 8],
    [3, 5, 14]
  ]
}
```

### `maxPriorityFeePerGas.min`

Take the min values from each set: `2, 0, 3`.
Find the min among them (min of all min values): `0`.
That's your answer: `0`

### `maxPriorityFeePerGas.median`

Take the median values from each set: `4, 7, 5`.
Find the median among them (median of all median values): `5`.
How did we do that?

- Sort the values in ascending order: `4, 5, 7`
- Pick the one in the middle: `5`.
  That's your answer: `5`

### `maxPriorityFeePerGas.max`

Take the max values from each set: `9, 8, 14`.
Find the max among them (max of all max values): `14`.
That's your answer: `14`

#### Note:

It's always the middle value if we have odd number of values.
What if we have an even number of values?
Then it'll be the average of two numbers in the middle.

### Result Of the Part 1:

```json
{
  "maxPriorityFeePerGas": {
    "min": 0,
    "median": 5,
    "max": 14
  }
}
```

## Part 2: `maxFeePerGas`

We need to find this:

```json
{
  "maxFeePerGas": {
    "min": "?",
    "median": "?",
    "max": "?"
  }
}
```

### Step 1: Find `min, median and max` of `baseFeePerGas`

Use the `baseFeePerGas` portion of our original json:

```json
{
  "baseFeePerGas": [96, 38, 50]
}
```

- Find `min` value among them: `38`
- Find `median` value among them: `50`
  (Sort them in the ascending order: `38, 50, 96`, then pick the one in the middle: `50`.)
- Find `max` value among them: `96`

#### Result of the Step 1:

```json
{
  "min": 38,
  "max": 96,
  "median": 50
}
```

### Step 2: Add the result of the Part 1: (`maxPriorityFeePerGas`) to the result of the Step 1 (only the median base fee is used here)

Result of the Part 1:

```json
{
  "maxPriorityFeePerGas": {
    "min": 0,
    "median": 5,
    "max": 14
  }
}
```

Result of the Step 1:

```json
{
  "min": 38,
  "median": 50,
  "max": 96
}
```

By adding them together, we get:
`0 + 50 = 50`
`5 + 50 = 55`
`14 + 50= 64`

### Result Of the Part 2:

```json
{
  "maxFeePerGas": {
    "min": 50,
    "median": 55,
    "max": 64
  }
}
```

## Part 3: Combine Parts 1 and 2 Together as a Response Object

```json
{
  "chain_id": "1",
  "eip-1559": {
    "maxPriorityFeePerGas": {
      "min": 0,
      "median": 5,
      "max": 14
    },
    "maxFeePerGas": {
      "min": 50,
      "median": 55,
      "max": 64
    }
  },
  // the same as `maxFeePerGas`
  "legacy": {
    "gasPrice": {
      "min": 50,
      "median": 55,
      "max": 64
    }
  }
}
```
