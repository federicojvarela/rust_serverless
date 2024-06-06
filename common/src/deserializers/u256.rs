use std::fmt;

use ethers::types::U256;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serializer,
};

struct U256Visitor;

impl<'de> Visitor<'de> for U256Visitor {
    type Value = U256;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(
            "string containing a decimal or hex number representing an unsigned 256 bits integer",
        )
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if let Some(striped_string) = s.strip_prefix("0x") {
            U256::from_str_radix(striped_string, 16)
                .map_err(|_| de::Error::custom("Invalid U256 value"))
        } else {
            U256::from_dec_str(s).map_err(|_| de::Error::custom("Invalid U256 value"))
        }
    }
}

pub fn unsigned_integer_256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(U256Visitor)
}

#[derive(Deserialize)]
struct OptionalU256(#[serde(deserialize_with = "unsigned_integer_256")] U256);

pub fn unsigned_integer_256_option<'de, D>(deserializer: D) -> Result<Option<U256>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<OptionalU256>::deserialize(deserializer)
        .map(|opt_wrapped: Option<OptionalU256>| opt_wrapped.map(|wrapped: OptionalU256| wrapped.0))
}

pub fn decimal_u256<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::U256;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct TestU256Deserialize {
        #[serde(deserialize_with = "unsigned_integer_256")]
        pub value: U256,
    }

    #[derive(Deserialize)]
    struct TestU256DeserializeOptional {
        #[serde(default, deserialize_with = "unsigned_integer_256_option")]
        pub value: Option<U256>,
    }

    #[derive(Serialize)]
    struct TestU256Serialize {
        #[serde(serialize_with = "decimal_u256")]
        pub value: U256,
    }

    #[test]
    fn test_deserialize_from_decimal_to_u256() {
        let to_deserialize = r#"{ "value": "51966" }"#;

        let value_deserialized: TestU256Deserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to U256");

        assert_eq!(value_deserialized.value.as_u64(), 51966);
    }

    #[test]
    fn test_deserialize_from_hexadecimal_to_u256() {
        let to_deserialize = r#"{ "value": "0xCAFE" }"#;

        let value_deserialized: TestU256Deserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to U256");

        assert_eq!(value_deserialized.value.as_u64(), 51966);
    }

    #[test]
    fn test_deserialize_from_hexadecimal_to_u256_validate_to_string() {
        let to_deserialize = r#"{ "value": "0xCAFE" }"#;

        let value_deserialized: TestU256Deserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to U256");

        assert_eq!(value_deserialized.value.to_string(), "51966");
    }

    #[test]
    fn test_deserialize_from_decimal_optional_to_u256() {
        let to_deserialize = r#"{ "value": "51966" }"#;

        let value_deserialized: TestU256DeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to U256");

        assert_eq!(value_deserialized.value.unwrap().as_u64(), 51966);
    }

    #[test]
    fn test_deserialize_from_hexadecimal_optional_to_u256() {
        let to_deserialize = r#"{ "value": "0xCAFE" }"#;

        let value_deserialized: TestU256DeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to U256");

        assert_eq!(value_deserialized.value.unwrap().as_u64(), 51966);
    }

    #[test]
    fn test_deserialize_decimal_from_optional_none_to_u256() {
        let to_deserialize = r#"{}"#;

        let value_deserialized: TestU256DeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to U256");

        assert!(value_deserialized.value.is_none());
    }

    #[test]
    fn test_serialize_to_decimal_from_u256() {
        let to_serialize = TestU256Serialize {
            value: U256::from_dec_str("51966").unwrap(),
        };
        let value_serialized =
            serde_json::to_string(&to_serialize).expect("Could not serialize U256");
        assert_eq!(value_serialized, r#"{"value":"51966"}"#);
    }

    #[test]
    fn test_serialize_to_hexadecimal_from_u256() {
        let to_serialize = TestU256Serialize {
            value: U256::from_dec_str("51966").unwrap(),
        };
        let value_serialized =
            serde_json::to_string(&to_serialize).expect("Could not serialize U256");

        assert_ne!(value_serialized, r#"{"value":"0xcafe"}"#);
    }
}
