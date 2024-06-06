use std::collections::hash_map::RandomState;
use std::collections::HashMap;

use anyhow::{anyhow, Error};
use chrono::Utc;
use rusoto_dynamodb::AttributeValue;
use serde::{Deserialize, Serialize};

use model::order::OrderState;

use crate::orders::{UpdateBlockNumberAndHash, UpdateStateAndLastModifiedTime};

#[derive(Deserialize, Debug, Serialize, Clone)]
/// This struct is meant to contain data other than "state" (new_state) and "last_modified_at".
/// The field "state" should be passed outside this struct and
/// the "last_modified_at" timestamp is computed at the update time.
/// Data like "nonce", "block_number", etc. can be passed from SMs and other methods in this struct.
pub struct UpdateOrderStatement {
    pub assignment_pairs: Option<HashMap<String, String>>, // ex: ("#nonce", ":nonce"), so we set #nonce = :nonce later
    pub attribute_names: Option<HashMap<String, String>>,  // ex: ("#nonce", "nonce")
    pub attribute_values: Option<HashMap<String, AttributeValue>>, // ex: (":nonce", AttributeValue("nonce");
}

pub fn build_update_order_statement_for_block_num_and_hash(
    block_number: u64,
    block_hash: String,
) -> Result<UpdateOrderStatement, Error> {
    let extra_attributes = serde_dynamo::to_item(UpdateBlockNumberAndHash {
        block_number,
        block_hash,
    })
    .map_err(|e| anyhow!(e).context("Error building UpdateBlockNumberAndHash expression"))?;

    Ok(UpdateOrderStatement {
        assignment_pairs: Some(HashMap::from([
            ("block_number".to_string(), ":block_number".to_string()),
            ("block_hash".to_string(), ":block_hash".to_string()),
        ])),
        attribute_names: None,
        attribute_values: Some(extra_attributes),
    })
}

pub fn compose_update_expression(update_order_statement: Option<UpdateOrderStatement>) -> String {
    let basic_update_expression =
        "SET #state = :state, last_modified_at = :last_modified_at".to_owned();
    match update_order_statement {
        None => basic_update_expression,
        Some(update_order_data) => {
            let mut all_assignments = vec![basic_update_expression];
            let pairs = update_order_data.assignment_pairs.unwrap_or_default();
            for entry in pairs {
                let assignment = format!("{:} = {:}", entry.0, entry.1);
                all_assignments.push(assignment);
            }
            all_assignments.join(", ")
        }
    }
}

pub fn build_current_states_condition_expression(
    expected_current_states: &[OrderState],
) -> (String, HashMap<String, AttributeValue>) {
    let states_attribute_values: HashMap<String, AttributeValue, RandomState> =
        HashMap::from_iter(expected_current_states.iter().map(|s| {
            (
                format!(":state_{}", s.as_str().to_lowercase()),
                AttributeValue {
                    s: Some(s.as_str().to_owned()),
                    ..Default::default()
                },
            )
        }));

    let states = states_attribute_values
        .keys()
        .map(|k| &**k)
        .collect::<Vec<_>>()
        .join(", ");

    (format!("#state IN ({states})"), states_attribute_values)
}

pub fn compose_attribute_values(
    new_state: OrderState,
    current_states_attribute_values: HashMap<String, AttributeValue>,
    update_order_statement: Option<UpdateOrderStatement>,
) -> Result<HashMap<String, AttributeValue>, anyhow::Error> {
    let mut attribute_values: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(UpdateStateAndLastModifiedTime {
            state: new_state,
            last_modified_at: Utc::now(),
        })
        .map_err(|e| anyhow!(e).context("Error building update order expression"))?;

    attribute_values.extend(current_states_attribute_values);

    if let Some(update_order_data) = update_order_statement {
        let extra_attribute_values = update_order_data.attribute_values.unwrap_or_default();
        attribute_values.extend(extra_attribute_values);
    };

    Ok(attribute_values)
}
