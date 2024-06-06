use std::fmt;

use serde::de::Error;
use serde::{de::Visitor, Deserializer};
struct StringOrH160Visitor;

impl<'de> Visitor<'de> for StringOrH160Visitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string or h160")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(s.to_owned())
    }
}

pub fn from_string_or_h160<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(StringOrH160Visitor)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;

    use super::*;
    use ethers::types::H160;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct TestDeserialize {
        #[serde(deserialize_with = "from_string_or_h160")]
        pub value: String,
    }

    #[test]
    fn test_deserialize_str() {
        let value_deserialized: TestDeserialize =
            serde_json::from_str(r#"{"value": "testaddress.eth"}"#).unwrap();
        assert_eq!("testaddress.eth", value_deserialized.value);
    }

    #[test]
    fn test_deserialize_h160() {
        let value_deserialized: TestDeserialize = serde_json::from_str(
            &json!({ "value": H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap() }).to_string(),
        )
        .unwrap();
        assert_eq!(ADDRESS_FOR_MOCK_REQUESTS, value_deserialized.value);
    }

    #[test]
    fn test_deserialize_string_0x0() {
        let value_deserialized: TestDeserialize =
            serde_json::from_str(&json!({ "value": "0x0" }).to_string()).unwrap();
        assert_eq!("0x0", value_deserialized.value);
    }
}
