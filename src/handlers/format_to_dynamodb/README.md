# Format to DynamoDB lambda

This lambda is intended to be used inside State Machines to convert "common" JSON format to
DynamoDB JSON format. This is useful because DynamoDB steps usually require DynamoDB JSON
format.

For example:

```json
{
    "positive_integer": 1,
    "negative_integer": -1,
    "floating_point": 1.15,
    "string": "test",
    "array": [1,2,3,4],
    "map": { "test": 2, "map": "yes" }
}
```

will be converted to:

```json
{ "M": {
    "positive_integer": { "N": "1" },
    "negative_integer": { "N": "-1" },
    "floating_point": { "N": "1.15" },
    "string": { "S": "test" },
    "array": { "L": [
        { "N": "1" },
        { "N": "2" },
        { "N": "3" },
        { "N": "4" }
    ] },
    "map": { "M": { "test": { "N": "2" }, "map": { "S": "yes" } } }
}}
```
