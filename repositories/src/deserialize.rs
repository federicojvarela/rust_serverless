use std::{collections::HashMap, error::Error};

use rusoto_dynamodb::AttributeValue;
use serde::Deserialize;

pub trait UnknownError {
    fn unknown<E: Error + Sync + Send + 'static>(e: E, context: Option<&'static str>) -> Self;
}

#[macro_export]
macro_rules! impl_unknown_error_trait {
    ($struct: ident) => {
        impl $crate::deserialize::UnknownError for $struct {
            fn unknown<E: std::error::Error + Sync + Send + 'static>(
                e: E,
                context: Option<&'static str>,
            ) -> Self {
                if let Some(ctx) = context {
                    Self::Unknown(anyhow::anyhow!(e).context(ctx))
                } else {
                    Self::Unknown(anyhow::anyhow!(e))
                }
            }
        }
    };
}

pub fn deserialize_from_dynamo<'a, O: Deserialize<'a>, E: UnknownError>(
    dynamo_object: HashMap<String, AttributeValue>,
) -> Result<O, E> {
    serde_dynamo::from_item(dynamo_object)
        .map_err(|e| E::unknown(e, Some("Error deserializing record")))
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_dynamo::AttributeValue;
    use serde_json::json;
    use std::collections::HashMap;

    #[derive(Serialize)]
    struct Subject {
        id: u8,
    }

    #[test]
    fn numbers_are_correctly_serialized_with_serde_dynamo() {
        let result: AttributeValue = serde_dynamo::to_attribute_value(Subject { id: 42 }).unwrap();

        assert_eq!(
            result,
            AttributeValue::M(HashMap::from([(
                String::from("id"),
                AttributeValue::N(String::from("42"))
            ),]))
        );
    }

    #[test]
    fn numbers_are_correctly_serialized_with_serde_dynamo_from_serde_json() {
        let json = json!({ "id": 42 });
        let result: AttributeValue = serde_dynamo::to_attribute_value(json).unwrap();

        assert_eq!(
            result,
            AttributeValue::M(HashMap::from([(
                String::from("id"),
                AttributeValue::N(String::from("42"))
            ),]))
        );
    }
}
