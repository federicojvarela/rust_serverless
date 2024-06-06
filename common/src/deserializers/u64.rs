use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{
    de::{self, Visitor},
    Deserializer,
};

struct U64Visitor;

impl U64Visitor {
    fn format_error<E, E2: Error>(e: E2) -> E
    where
        E: de::Error,
    {
        de::Error::custom(format!("Invalid u64 value: {e:#}"))
    }
}

impl<'de> Visitor<'de> for U64Visitor {
    type Value = u64;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string containing a 64 bit number")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if let Some(striped_string) = s.strip_prefix("0x") {
            u64::from_str_radix(striped_string, 16).map_err(U64Visitor::format_error)
        } else {
            u64::from_str(s).map_err(U64Visitor::format_error)
        }
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(u64::from(v))
    }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(u64::from(v))
    }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(u64::from(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v)
    }

    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(v).map_err(U64Visitor::format_error)
    }

    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(v).map_err(U64Visitor::format_error)
    }

    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(v).map_err(U64Visitor::format_error)
    }

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(v).map_err(U64Visitor::format_error)
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(v).map_err(U64Visitor::format_error)
    }

    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u64::try_from(v).map_err(U64Visitor::format_error)
    }
}

pub fn str_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(U64Visitor)
}

pub fn maybe_from_str_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(U64Visitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestU64Deserialize {
        #[serde(deserialize_with = "str_u64")]
        pub value: u64,
    }

    #[test]
    fn test_deserialize_from_string_to_u64() {
        let to_deserialize = r#"{"value": "1"}"#;
        let value_deserialized: TestU64Deserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize String to u64");
        assert_eq!(format!("{:?}", value_deserialized.value), "1");
    }

    #[test]
    fn test_deserialize_from_hexadecilam_string_to_u64() {
        let to_deserialize = r#"{"value": "0x1"}"#;
        let value_deserialized: TestU64Deserialize = serde_json::from_str(to_deserialize)
            .expect("Could not deserialize Hexadecimal String to u64");
        assert_eq!(format!("{:?}", value_deserialized.value), "1");
    }

    #[derive(Deserialize)]
    struct TestU64DeserializeMaybe {
        #[serde(deserialize_with = "maybe_from_str_u64")]
        pub value: u64,
    }

    #[test]
    fn test_deserialize_from_maybe_string_to_u64() {
        let to_deserialize = r#"{"value": "1"}"#;
        let value_deserialized: TestU64DeserializeMaybe =
            serde_json::from_str(to_deserialize).expect("Could not deserialize String to u64");
        assert_eq!(format!("{:?}", value_deserialized.value), "1");
    }

    #[test]
    fn test_deserialize_from_maybe_hexadecimal_string_to_u64() {
        let to_deserialize = r#"{"value": "0x1"}"#;
        let value_deserialized: TestU64DeserializeMaybe = serde_json::from_str(to_deserialize)
            .expect("Could not deserialize Hexadecimal String to u64");
        assert_eq!(format!("{:?}", value_deserialized.value), "1");
    }

    #[test]
    fn test_deserialize_from_maybe_number_to_u64() {
        let to_deserialize = r#"{"value": 42}"#;
        let value_deserialized: TestU64DeserializeMaybe = serde_json::from_str(to_deserialize)
            .expect("Could not deserialize number String to u64");
        assert_eq!(value_deserialized.value, 42);
    }
}
