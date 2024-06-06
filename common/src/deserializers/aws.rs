use rusoto_core::Region;
use serde::de::Visitor;
use serde::{de, Deserializer};
use std::fmt;
use std::str::FromStr;

struct AwsRegionVisitor;

impl<'de> Visitor<'de> for AwsRegionVisitor {
    type Value = Region;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string representing a valid AWS Region")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Region::from_str(s).map_err(|_| de::Error::custom("Invalid AWS Region"))
    }
}

pub fn aws_region<'de, D>(deserializer: D) -> Result<Region, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(AwsRegionVisitor)
}
