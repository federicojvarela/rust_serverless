use async_trait::async_trait;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::OrchestrationError,
    result::Result,
};
use serde_dynamo::AttributeValue;
use serde_json::{Map, Number, Value};

pub struct FormatFromDynamoDB;

#[async_trait]
impl Lambda for FormatFromDynamoDB {
    type PersistedMemory = ();
    type InputBody = AttributeValue;

    type Output = Value;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        Ok(())
    }

    async fn run(request: Self::InputBody, _state: &Self::PersistedMemory) -> Result<Self::Output> {
        let response = translate_from_dynamo_json(request);

        Ok(response)
    }
}

pub fn translate_from_dynamo_json(value: AttributeValue) -> Value {
    match value {
        AttributeValue::Null(true) => Value::Null,
        AttributeValue::Bool(b) => Value::Bool(b),
        AttributeValue::N(ref n) => {
            if let Ok(number) = &n.parse::<u64>() {
                Value::Number(Number::from(*number))
            } else if let Ok(number) = &n.parse::<i64>() {
                Value::Number(Number::from(*number))
            } else if let Ok(number) = &n.parse::<f64>() {
                if let Some(number) = Number::from_f64(*number) {
                    Value::Number(number)
                } else {
                    tracing::error!(
                        payload = ?value,
                        "Parsing error, {} is an invalid float",
                        number
                    );
                    panic!("{number} is an invalid float")
                }
            } else {
                tracing::error!(
                    payload = ?value,
                    "Parsing error, {} is not a valid number",
                    n
                );
                panic!("{n} is not a valid number");
            }
        }
        AttributeValue::S(s) => Value::String(s),
        AttributeValue::L(a) => {
            Value::Array(a.into_iter().map(translate_from_dynamo_json).collect())
        }
        AttributeValue::M(m) => {
            let mut map: Map<String, Value> = Map::new();
            for (key, val) in m {
                map.insert(key, translate_from_dynamo_json(val));
            }
            Value::Object(map)
        }
        ref t => {
            tracing::error!(
                payload = ?value,
                "Invalid type to convert to common JSON: {t:?}",
            );
            panic!("Invalid type to convert to common JSON: {t:?}")
        }
    }
}

lambda_main!(FormatFromDynamoDB);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conversion_from_dynamodb_format_ok() {
        let dynamodb_json = r#"{ "M": {
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
        }}"#;

        let expected = json!({
            "positive_integer": 1,
            "negative_integer": -1,
            "floating_point": 1.15,
            "string": "test",
            "array": [1,2,3,4],
            "map": { "test": 2, "map": "yes" }
        });

        let attributes: AttributeValue = serde_json::from_str(dynamodb_json).unwrap();
        let translation = translate_from_dynamo_json(attributes);

        assert_eq!(expected, translation);
    }
}
