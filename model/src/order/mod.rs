use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::order::policy::Policy;

pub use self::order_data::{
    GenericOrderData, OrderData, OrderTransaction, SharedOrderData, SignatureOrderData,
    SponsorAddresses,
};

pub mod helpers;
mod order_data;
pub mod policy;

#[derive(Debug, Error)]
pub enum OrderStatusError {
    #[error(r#"An order data of type "{0}" can't be extracted from an order of type "{1}""#)]
    OrderDataExtractionFailed(&'static str, OrderType),

    #[error("order data not found in order of type {0}")]
    OrderDataNotFound(OrderType),

    #[error("Unable to deserialize order data")]
    OrderDataDeserialization(#[from] serde_json::Error),
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct OrderPK {
    pub order_id: String,
}

#[derive(Deserialize, Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    #[serde(rename = "KEY_CREATION_ORDER")]
    KeyCreation,
    #[serde(rename = "SIGNATURE_ORDER")]
    Signature,
    #[serde(rename = "SPEEDUP_ORDER")]
    SpeedUp,
    #[serde(rename = "SPONSORED_ORDER")]
    Sponsored,
    #[serde(rename = "CANCELLATION_ORDER")]
    Cancellation,
}

impl OrderType {
    /// Order types that may result in a transaction being sent to a chain.
    /// Right now it's just all non-key-creation order types
    pub const ORDER_TYPES_WITH_TXN: [OrderType; 4] = [
        OrderType::Signature,
        OrderType::SpeedUp,
        OrderType::Sponsored,
        OrderType::Cancellation,
    ];
}

#[derive(Deserialize, Debug, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderState {
    Cancelled,
    Completed,
    CompletedWithError,
    ApproversReviewed,
    Dropped,
    Error,
    NotSigned,
    NotSubmitted,
    Received,
    Reorged,
    Replaced,
    SelectedForSigning,
    Signed,
    Submitted,
}

impl OrderState {
    /// Pending states are states that are not terminal.
    pub const PENDING_ORDER_STATES: [OrderState; 5] = [
        OrderState::Received,
        OrderState::ApproversReviewed,
        OrderState::SelectedForSigning,
        OrderState::Signed,
        OrderState::Submitted,
    ];

    /// If an order is in a locking state, means that the order from address is locked for sure.
    const LOCKING_ORDER_STATES: [OrderState; 3] = [
        OrderState::SelectedForSigning,
        OrderState::Signed,
        OrderState::Submitted,
    ];

    /// Returns true if the order address is locked if an order that belong to that address is in
    /// self `state`.
    pub fn is_locking_state(&self) -> bool {
        Self::LOCKING_ORDER_STATES.contains(self)
    }

    /// Returns true if the order is in a pending state (non-terminal)
    pub fn is_pending_state(&self) -> bool {
        Self::PENDING_ORDER_STATES.contains(self)
    }

    /// Gets the possible states where the order can transition to `next_state`.
    pub fn get_possible_current_state<'a>(next_state: OrderState) -> &'a [OrderState] {
        match next_state {
            // Terminal States
            OrderState::Cancelled => &[
                OrderState::Received,
                OrderState::ApproversReviewed,
                OrderState::SelectedForSigning,
                OrderState::Signed,
            ],
            OrderState::Completed => &[OrderState::Reorged, OrderState::Submitted],
            OrderState::CompletedWithError => &[OrderState::Reorged, OrderState::Submitted],
            OrderState::Dropped => &[OrderState::Reorged, OrderState::Submitted],
            OrderState::NotSigned => &[OrderState::SelectedForSigning],
            OrderState::NotSubmitted => &[OrderState::Signed],
            OrderState::Replaced => &[
                OrderState::Dropped,
                OrderState::Reorged,
                OrderState::Submitted,
            ],
            OrderState::Error => &[
                Self::ApproversReviewed,
                Self::Received,
                Self::Reorged,
                Self::SelectedForSigning,
                Self::Signed,
                Self::Submitted,
            ],

            // Non-terminal states
            OrderState::Submitted => &[OrderState::Signed],
            OrderState::Received => &[],
            OrderState::ApproversReviewed => &[OrderState::Received],
            OrderState::SelectedForSigning => &[OrderState::ApproversReviewed],
            OrderState::Signed => &[OrderState::SelectedForSigning],
            OrderState::Reorged => &[OrderState::Submitted],
        }
    }
}

impl Display for OrderState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let printable = self.as_str();
        write!(f, "{}", printable)
    }
}

impl Display for OrderType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let printable = self.as_str();
        write!(f, "{}", printable)
    }
}

impl OrderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderType::KeyCreation => "KEY_CREATION_ORDER",
            OrderType::Signature => "SIGNATURE_ORDER",
            OrderType::SpeedUp => "SPEEDUP_ORDER",
            OrderType::Sponsored => "SPONSORED_ORDER",
            OrderType::Cancellation => "CANCELLATION_ORDER",
        }
    }
}

impl OrderState {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderState::Cancelled => "CANCELLED",
            OrderState::Completed => "COMPLETED",
            OrderState::CompletedWithError => "COMPLETED_WITH_ERROR",
            OrderState::ApproversReviewed => "APPROVERS_REVIEWED",
            OrderState::Dropped => "DROPPED",
            OrderState::Error => "ERROR",
            OrderState::NotSigned => "NOT_SIGNED",
            OrderState::NotSubmitted => "NOT_SUBMITTED",
            OrderState::Received => "RECEIVED",
            OrderState::Reorged => "REORGED",
            OrderState::Replaced => "REPLACED",
            OrderState::SelectedForSigning => "SELECTED_FOR_SIGNING",
            OrderState::Signed => "SIGNED",
            OrderState::Submitted => "SUBMITTED",
        }
    }
}

impl FromStr for OrderState {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_uppercase(); // Convert the input to uppercase for case-insensitive comparison
        match s.as_str() {
            "CANCELLED" => Ok(OrderState::Cancelled),
            "COMPLETED" => Ok(OrderState::Completed),
            "COMPLETED_WITH_ERROR" => Ok(OrderState::CompletedWithError),
            "APPROVERS_REVIEWED" => Ok(OrderState::ApproversReviewed),
            "DROPPED" => Ok(OrderState::Dropped),
            "ERROR" => Ok(OrderState::Error),
            "NOT_SUBMITTED" => Ok(OrderState::NotSubmitted),
            "RECEIVED" => Ok(OrderState::Received),
            "REORGED" => Ok(OrderState::Reorged),
            "REPLACED" => Ok(OrderState::Replaced),
            "SELECTED_FOR_SIGNING" => Ok(OrderState::SelectedForSigning),
            "SIGNED" => Ok(OrderState::Signed),
            "SUBMITTED" => Ok(OrderState::Submitted),
            other => Err(anyhow!("Not supported OrderState variant: {other}")),
        }
    }
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct OrderStatus {
    pub order_id: Uuid,
    pub order_version: String,
    pub state: OrderState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    pub data: GenericOrderData,
    pub created_at: DateTime<Utc>,
    pub order_type: OrderType,
    pub last_modified_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub replaced_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub replaces: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<Policy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_requested: Option<bool>,
}

impl OrderStatus {
    pub fn extract_signature_data(
        &self,
    ) -> Result<OrderData<SignatureOrderData>, OrderStatusError> {
        match self.order_type {
            OrderType::Signature
            | OrderType::Sponsored
            | OrderType::SpeedUp
            | OrderType::Cancellation => self.signature_data(),
            OrderType::KeyCreation => Err(OrderStatusError::OrderDataExtractionFailed(
                "signature",
                self.order_type,
            )),
        }
    }

    pub fn signature_data(&self) -> Result<OrderData<SignatureOrderData>, OrderStatusError> {
        let data = serde_json::to_value(&self.data)?;
        Ok(serde_json::from_value::<OrderData<SignatureOrderData>>(
            data,
        )?)
    }
}

#[derive(Deserialize, Debug, Serialize, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct OrderSummary {
    pub order_id: Uuid,
    pub state: OrderState,
    pub created_at: DateTime<Utc>,
    pub order_type: OrderType,
    #[serde(default)]
    pub cancellation_requested: bool,
}
