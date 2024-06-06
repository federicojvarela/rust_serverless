use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{self, Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct CacheItem<T> {
    pub sk: String,
    pub pk: DataType,
    #[serde(flatten)]
    pub data: T,
    pub created_at: DateTime<Utc>,
    pub expires_at: i64,
}

#[derive(Deserialize, Debug, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataType {
    FtMetadata,
    AddressLock,
}

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let printable = self.as_str();
        write!(f, "{}", printable)
    }
}

impl DataType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataType::FtMetadata => "FT_METADATA",
            DataType::AddressLock => "ADDRESS_LOCK",
        }
    }
}

impl FromStr for DataType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_uppercase(); // Convert the input to uppercase for case-insensitive comparison
        match s.as_str() {
            "FT_METADATA" => Ok(DataType::FtMetadata),
            other => Err(anyhow!("Not supported DataType variant: {other}")),
        }
    }
}

pub type GenericJsonCache = CacheItem<serde_json::Value>;
