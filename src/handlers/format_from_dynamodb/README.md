# Format from DynamoDB lambda

This lambda is intended to be used inside State Machines to convert DynamoDB JSON format to
"common" JSON format. This is useful because DynamoDB steps usually return in DynamoDB JSON
format.

For example:

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

will be converted to:

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
