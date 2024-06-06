use std::fmt;

use serde::de::Error;
use serde::{de::Visitor, Deserializer};
struct StringOrNumberVisitor;

impl<'de> Visitor<'de> for StringOrNumberVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string or number")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(s.to_owned())
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v.to_string())
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v.to_string())
    }
}

pub fn from_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(StringOrNumberVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct TestDeserialize {
        #[serde(deserialize_with = "from_string_or_number")]
        pub value: String,
    }

    #[test]
    fn test_deserialize_str() {
        let value_deserialized: TestDeserialize =
            serde_json::from_str(r#"{"value": "10"}"#).unwrap();
        assert_eq!("10", value_deserialized.value);
    }

    #[test]
    fn test_deserialize_numbers() {
        let value_deserialized: TestDeserialize =
            serde_json::from_str(&json!({ "value": i64::MIN }).to_string()).unwrap();
        assert_eq!(i64::MIN.to_string(), value_deserialized.value);

        let value_deserialized: TestDeserialize =
            serde_json::from_str(&json!({"value": 0}).to_string()).unwrap();
        assert_eq!("0", value_deserialized.value);

        let value_deserialized: TestDeserialize =
            serde_json::from_str(&json!({ "value": u64::MAX }).to_string()).unwrap();
        assert_eq!(u64::MAX.to_string(), value_deserialized.value);
    }
}
