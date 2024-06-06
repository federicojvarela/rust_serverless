use std::fmt;

use ethers::types::Bytes;
use hex::FromHex;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

struct BytesVisitor;

impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Bytes;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("hex string with 0x prefix")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !s.starts_with("0x") {
            return Err(de::Error::custom("expected 0x"));
        }

        let s_no_prefix = &s[2..]; // Remove the "0x" prefix
        let vec = Vec::<u8>::from_hex(s_no_prefix).map_err(|e| {
            let e = anyhow::anyhow!(e).context("Invalid Bytes value");
            de::Error::custom(format!("{e:#}"))
        })?;
        Ok(vec.into())
    }
}

pub fn bytes<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(BytesVisitor)
}

#[derive(Deserialize)]
struct OptionalBytes(#[serde(deserialize_with = "bytes")] Bytes);

pub fn bytes_option<'de, D>(deserializer: D) -> Result<Option<Bytes>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<OptionalBytes>::deserialize(deserializer).map(|opt_wrapped: Option<OptionalBytes>| {
        opt_wrapped.map(|wrapped: OptionalBytes| wrapped.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestBytesDeserialize {
        #[serde(deserialize_with = "bytes")]
        pub value: Bytes,
    }

    #[derive(Deserialize)]
    struct TestBytesDeserializeOptional {
        #[serde(default, deserialize_with = "bytes_option")]
        pub value: Option<Bytes>,
    }

    #[test]
    fn test_cannot_deserialize_from_non_hex_to_bytes() {
        let to_deserialize = r#"{ "value": "51966" }"#;
        let result: Result<TestBytesDeserialize, serde_json::Error> =
            serde_json::from_str(to_deserialize);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_from_hexadecimal_to_bytes() {
        let to_deserialize = r#"{ "value": "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026" }"#;

        let value_deserialized: TestBytesDeserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to Bytes");

        assert_eq!(
            format!("{}", value_deserialized.value),
            "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026"
        );
    }

    #[test]
    fn test_cannot_deserialize_from_non_hex_option_to_bytes() {
        let to_deserialize = r#"{ "value": "51966" }"#;
        let result: Result<TestBytesDeserializeOptional, serde_json::Error> =
            serde_json::from_str(to_deserialize);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_from_hexadecimal_option_to_bytes() {
        let to_deserialize = r#"{ "value": "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026" }"#;

        let value_deserialized: TestBytesDeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to Bytes");

        assert_eq!(
            format!("{}", value_deserialized.value.unwrap()),
            "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026"
        );
    }
    #[test]
    fn test_deserialize_from_hexadecimal_option_none_to_bytes() {
        let to_deserialize = r#"{}"#;

        let value_deserialized: TestBytesDeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to Bytes");

        assert!(value_deserialized.value.is_none());
    }
}
