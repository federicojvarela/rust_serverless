use serde::{
    de::{DeserializeOwned, Error},
    Deserializer,
};

struct JsonStringVisitor;

impl<'de> serde::de::Visitor<'de> for JsonStringVisitor {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string containing json data")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::from_str(v).map_err(E::custom)
    }
}

pub fn deserialize_json_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let des = deserializer.deserialize_any(JsonStringVisitor)?;
    serde_json::from_value::<T>(des).map_err(D::Error::custom)
}
