use std::collections::HashMap;

use async_trait::async_trait;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::OrchestrationError,
    result::Result,
};
use serde_dynamo::AttributeValue;
use serde_json::Value;

pub struct FormatToDynamoDB;

#[async_trait]
impl Lambda for FormatToDynamoDB {
    type PersistedMemory = ();
    type InputBody = Value;
    type Output = Value;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        Ok(())
    }

    async fn run(request: Self::InputBody, _state: &Self::PersistedMemory) -> Result<Self::Output> {
        let dynamodb_json = translate_to_dynamo_json(request);
        let response = serde_json::to_value(dynamodb_json)
            .map_err(|e| OrchestrationError::from(anyhow::anyhow!(e)))?;

        Ok(response)
    }
}

pub fn translate_to_dynamo_json(value: Value) -> AttributeValue {
    match value {
        Value::Null => AttributeValue::Null(true),
        Value::Bool(b) => AttributeValue::Bool(b),
        Value::Number(n) => AttributeValue::N(n.to_string()),
        Value::String(s) => AttributeValue::S(s),
        Value::Array(a) => AttributeValue::L(a.into_iter().map(translate_to_dynamo_json).collect()),
        Value::Object(m) => {
            let mut map: HashMap<String, AttributeValue> = HashMap::new();
            for (key, val) in m {
                map.insert(key, translate_to_dynamo_json(val));
            }
            AttributeValue::M(map)
        }
    }
}

lambda_main!(FormatToDynamoDB);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conversion_to_dynamodb_format_ok() {
        let common_json = json!({
            "positive_integer": 1,
            "negative_integer": -1,
            "floating_point": 1.15,
            "string": "test",
            "array": [1,2,3,4],
            "map": { "test": 2, "map": "yes" }
        });

        let expected: AttributeValue = serde_json::from_str(
            r#"{ "M": {
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
        }}"#,
        )
        .unwrap();

        let translation = translate_to_dynamo_json(common_json);

        assert_eq!(expected, translation);
    }
}
