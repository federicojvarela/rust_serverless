use ethers::types::H160;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::deserializers::h160::h160;
use mpc_signature_sm::dtos::requests::transaction_request::TransactionRequest;

#[derive(Deserialize, Serialize, Debug)]
pub struct ContextualDataRequest {
    pub order_id: Uuid,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ApproversRequest {
    pub contextual_data: ContextualDataRequest,

    pub transaction: TransactionRequest,

    #[serde(default, deserialize_with = "h160")]
    pub from: H160,

    pub approval_status: Option<i32>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS,
    };
    use model::order::helpers::sponsored_typed_data;

    use super::*;

    #[test]
    fn it_deserializes_an_approvers_request() {
        let request = format!(
            r#"{{
        "transaction":{{
                "to":"{}",
                "gas":"300000",
                "gas_price":"300000000",
                "value":"111111",
                "nonce":"15",
                "data":"0x6406516041610651325106165165106516169610",
                "chain_id":1
            }},
            "from": "0xFF6A5DB899FB29F67A224CDA089572C2BC5A7A5E",
            "contextual_data":{{
                "order_id":"{}"
            }}
        }}"#,
            ADDRESS_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS
        );

        let _request: ApproversRequest = serde_json::from_str(request.as_str()).unwrap();
    }

    #[test]
    fn it_deserializes_an_approvers_sponsored_request() {
        let request = json!( {
            "transaction": {
                "chain_id":1,
                "typed_data": sponsored_typed_data()
            },
            "from": "0xFF6A5DB899FB29F67A224CDA089572C2BC5A7A5E",
            "contextual_data":{
                "order_id":ORDER_ID_FOR_MOCK_REQUESTS
            }
        }
        );

        let _request: ApproversRequest =
            serde_json::from_str(request.to_string().as_str()).unwrap();
    }
}
