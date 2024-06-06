use common::deserializers::aws::aws_region;
use rusoto_core::Region;
use serde::{self, Deserialize};

#[derive(Deserialize)]
pub struct EbConfig {
    /// Current AWS region.
    #[serde(deserialize_with = "aws_region")]
    pub aws_region: Region,

    /// Name of the event bus where we'll publish transactions
    pub event_bridge_event_bus_name: String,
}
