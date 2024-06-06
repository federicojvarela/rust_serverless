use serde::{Deserialize, Serialize};

use chrono::{serde::ts_milliseconds, DateTime, Utc};
use std::default::Default;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Event<T> {
    pub payload: T,
    pub context: EventContext,
}

/// Supplemental event context that is not provided by amazon by default.
#[derive(Debug, Deserialize, Serialize, Default, PartialEq, Clone)]
pub struct EventContext {
    pub order_id: Uuid,
    #[serde(with = "ts_milliseconds", default = "Utc::now")]
    pub order_timestamp: DateTime<Utc>,
}

impl EventContext {
    /// Creates a new event from this context.
    ///
    /// # Arguments
    ///
    /// * `payload` - New event's payload
    pub fn create_new_event_from_current<T>(self, payload: T) -> Event<T> {
        Event {
            payload,
            context: self,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LambdaResponseEvent {
    pub context: EventContext,
}

impl<T> Event<T> {
    pub fn test_event_from(payload: T) -> Event<T> {
        Event {
            payload,
            context: EventContext {
                order_id: Uuid::new_v4(),
                order_timestamp: Utc::now(),
            },
        }
    }
}
