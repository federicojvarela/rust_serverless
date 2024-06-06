use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum MaestroAuthorizingEntityLevel {
    Domain,
    Tenant,
}

impl fmt::Display for MaestroAuthorizingEntityLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MaestroAuthorizingEntityLevel::Domain => {
                write!(f, "Domain")
            }
            MaestroAuthorizingEntityLevel::Tenant => {
                write!(f, "Tenant")
            }
        }
    }
}

impl FromStr for MaestroAuthorizingEntityLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_uppercase(); // Convert the input to uppercase for case-insensitive comparison
        match s.as_str() {
            "DOMAIN" => Ok(MaestroAuthorizingEntityLevel::Domain),
            "TENANT" => Ok(MaestroAuthorizingEntityLevel::Tenant),
            other => Err(anyhow!(
                "Not supported MaestroAuthorizingEntityLevel variant: {other}"
            )),
        }
    }
}
