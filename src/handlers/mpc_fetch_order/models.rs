use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use model::order::policy::Policy;
use model::order::{OrderStatus, OrderType};
use serde::{self, Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize, Debug, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct OrderResponse {
    pub order_id: Uuid,
    pub order_version: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    pub data: MpcOrderDataResponse,
    pub created_at: DateTime<Utc>,
    pub order_type: OrderType,
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug, Default, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct MpcOrderDataResponse {
    pub client_id: String,
    #[serde(flatten)]
    pub data: BTreeMap<String, Value>,
}

impl From<OrderStatus> for OrderResponse {
    fn from(order: OrderStatus) -> Self {
        let lookup = serde_json::from_value(order.data.data).map_or_else(
            |_| HashMap::new(),
            |mut map: HashMap<String, Value>| {
                if let Some(transaction_hash) = order.transaction_hash {
                    map.insert(
                        "transaction_hash".to_owned(),
                        Value::String(transaction_hash),
                    );
                }

                if let Some(policy) = order.policy {
                    map.insert(
                        "approvals".to_owned(),
                        Value::Object(build_approvers_response_from(&policy)),
                    );
                }

                // filter out key_id to not expose to client
                map.remove("key_id");
                map
            },
        );
        // order it so the data field doesn't keep changing the order everytime it's fetched
        let lookup: BTreeMap<_, _> = lookup.into_iter().collect();

        OrderResponse {
            order_id: order.order_id,
            order_version: order.order_version,
            state: order.state.to_string(),
            data: MpcOrderDataResponse {
                client_id: order.data.shared_data.client_id,
                data: lookup,
            },
            created_at: order.created_at,
            order_type: order.order_type,
            last_modified_at: order.last_modified_at,
            error: order.error,
        }
    }
}

fn build_approvers_response_from(policy: &Policy) -> serde_json::Map<String, Value> {
    policy
        .approvals
        .iter()
        .map(|approver| {
            let status = match approver.response.as_ref() {
                None => "PENDING",
                Some(response) => {
                    if response.approval_status == 1 {
                        "APPROVED"
                    } else {
                        "REJECTED"
                    }
                }
            };
            (approver.name.clone(), Value::String(status.to_owned()))
        })
        .collect()
}
