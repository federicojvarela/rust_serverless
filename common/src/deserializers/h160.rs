use std::fmt;
use std::str::FromStr;

use crate::test_tools::http::constants::ADDRESS_ZERO;
use ethers::types::{Address, H160};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

struct H160VecVisitor;
struct H160Visitor;

impl<'de> Visitor<'de> for H160Visitor {
    type Value = H160;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string containing a 160 bit hex number")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.to_lowercase() == ADDRESS_ZERO.to_lowercase() {
            Ok(H160::zero())
        } else {
            Address::from_str(value).map_err(|e| {
                let e = anyhow::anyhow!(e).context("Invalid H160 value");
                de::Error::custom(format!("{e:#}"))
            })
        }
    }
}

impl<'de> de::Visitor<'de> for H160VecVisitor {
    type Value = Vec<H160>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("list of string containing a 160 bit hex number")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let retval = if value.to_lowercase() == ADDRESS_ZERO.to_lowercase() {
            H160::zero()
        } else {
            Address::from_str(value).map_err(|e| {
                let e = anyhow::anyhow!(e).context("Invalid H160 value");
                de::Error::custom(format!("{e:#}"))
            })?
        };
        Ok(vec![retval])
    }

    fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
    where
        S: de::SeqAccess<'de>,
    {
        Deserialize::deserialize(de::value::SeqAccessDeserializer::new(visitor))
    }
}

pub fn from_array_h160<'de, D>(deserializer: D) -> Result<Vec<H160>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(H160VecVisitor)
}

pub fn h160<'de, D>(deserializer: D) -> Result<H160, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(H160Visitor)
}

#[derive(Deserialize)]
struct OptionalH160(#[serde(deserialize_with = "h160")] H160);

pub fn h160_option<'de, D>(deserializer: D) -> Result<Option<H160>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<OptionalH160>::deserialize(deserializer)
        .map(|opt_wrapped: Option<OptionalH160>| opt_wrapped.map(|wrapped: OptionalH160| wrapped.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestH160Deserialize {
        #[serde(deserialize_with = "h160")]
        pub value: H160,
    }

    #[derive(Deserialize)]
    struct TestH160DeserializeOptional {
        #[serde(default, deserialize_with = "h160_option")]
        pub value: Option<H160>,
    }

    #[derive(Deserialize)]
    struct TestH160DeserializeArray {
        #[serde(default, deserialize_with = "from_array_h160")]
        pub value: Vec<H160>,
    }

    #[test]
    fn test_cannot_deserialize_from_non_hex_to_h160() {
        let to_deserialize = r#"{ "value": "51966" }"#;
        let result: Result<TestH160Deserialize, serde_json::Error> =
            serde_json::from_str(to_deserialize);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_from_hexadecimal_to_h160() {
        let to_deserialize = r#"{ "value": "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026" }"#;

        let value_deserialized: TestH160Deserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to H160");

        assert_eq!(
            format!("{:?}", value_deserialized.value),
            "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026"
        );
    }

    #[test]
    fn test_deserialize_from_address_zero_to_h160() {
        let to_deserialize = r#"{ "value": "0x0" }"#;
        let value_deserialized: TestH160Deserialize =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to H160");
        assert_eq!(
            format!("{:?}", value_deserialized.value),
            "0x0000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_deserialize_from_array_to_h160() {
        let to_deserialize = r#"{ "value": ["0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026","0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026"] }"#;

        let value_deserialized: TestH160DeserializeArray =
            serde_json::from_str(to_deserialize).unwrap();

        assert_eq!(2, value_deserialized.value.len());
        assert_eq!(
            format!("{:?}", value_deserialized.value[1]),
            "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026"
        );
    }

    #[test]
    fn test_cannot_deserialize_from_non_hex_array_to_h160() {
        let to_deserialize = r#"{ "value": "51966" }"#;
        let result: Result<TestH160DeserializeArray, serde_json::Error> =
            serde_json::from_str(to_deserialize);
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_deserialize_from_non_hex_option_to_h160() {
        let to_deserialize = r#"{ "value": "51966" }"#;
        let result: Result<TestH160DeserializeOptional, serde_json::Error> =
            serde_json::from_str(to_deserialize);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_from_hexadecimal_option_to_h160() {
        let to_deserialize = r#"{ "value": "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026" }"#;

        let value_deserialized: TestH160DeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to H160");

        assert_eq!(
            format!("{:?}", value_deserialized.value.unwrap()),
            "0xeed5ca15f7339ee8f96f9a3e28bc71c1bbff7026"
        );
    }
    #[test]
    fn test_deserialize_from_hexadecimal_option_none_to_h160() {
        let to_deserialize = r#"{}"#;

        let value_deserialized: TestH160DeserializeOptional =
            serde_json::from_str(to_deserialize).expect("Could not deserialize to H160");

        assert!(value_deserialized.value.is_none());
    }
}
