mod dtos;

use crate::dtos::{ProcessApproverRequest, ProcessApproverResponse};
use anyhow::anyhow;
use async_trait::async_trait;
use model::order::policy::{ApprovalResponse, Policy};
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::OrchestrationError,
};

pub struct UpdateOrderApproverResponse;

#[async_trait]
impl Lambda for UpdateOrderApproverResponse {
    type PersistedMemory = ();
    type InputBody = ProcessApproverRequest;
    type Output = ProcessApproverResponse;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        Ok(())
    }

    async fn run(
        request: Self::InputBody,
        _state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        validate_response_can_be_added(&request.fetched.order.policy, &request.approval_response)?;

        let response = update_approval(request)?;

        Ok(response)
    }
}

fn validate_response_can_be_added(
    policy: &Policy,
    response: &ApprovalResponse,
) -> Result<(), anyhow::Error> {
    let _ = policy
        .approvals
        .iter()
        .find(|approval| approval.name == response.approver_name)
        .ok_or_else(|| {
            anyhow!(
                "Order does not expect a response from approver named: {}",
                response.approver_name
            )
        })?;

    Ok(())
}

pub fn update_approval(
    request: ProcessApproverRequest,
) -> Result<ProcessApproverResponse, anyhow::Error> {
    let order_id = request.approval_response.order_id.clone();
    let mut policy = request.fetched.order.policy;
    let approver_name = request.approval_response.approver_name;
    let error_message =
        format!("Error could not find approval with matching approver_name: {approver_name}");

    let index = policy
        .approvals
        .iter()
        .position(|approval| approval.name == approver_name)
        .ok_or_else(|| {
            anyhow!("{error_message}. Failed to update approval for order: {order_id}")
        })?;

    let approval = &mut policy.approvals[index];
    if let Some(response) = &approval.response {
        tracing::warn!(
            order_id = ?order_id,
            "Order {order_id} already had a response from approver named: {}, this might be due to a retry",
            response.approver_name
        );
    }

    approval.response = Some(ApprovalResponse {
        order_id: request.approval_response.order_id.clone(),
        status_reason: request.approval_response.status_reason.clone(),
        approval_status: request.approval_response.approval_status,
        approver_name: approver_name.clone(),
        metadata: request.approval_response.metadata.clone(),
        metadata_signature: request.approval_response.metadata_signature.clone(),
    });

    Ok(ProcessApproverResponse { policy })
}

lambda_main!(UpdateOrderApproverResponse);

#[cfg(test)]
mod tests {
    use crate::dtos::{Fetched, FetchedOrder, ProcessApproverRequest, ProcessApproverResponse};
    use crate::UpdateOrderApproverResponse;
    use model::order::policy::{Approval, ApprovalResponse, Policy};
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::result::error::OrchestrationError;
    use uuid::Uuid;

    const POLICY_NAME: &str = "DefaultApprover";
    const APPROVER_NAME: &str = "Approver_1";

    #[tokio::test]
    async fn approver_response_already_recorded() {
        let request = ProcessApproverRequest {
            approval_response: ApprovalResponse {
                order_id: String::default(),
                status_reason: String::default(),
                approval_status: 0,
                approver_name: APPROVER_NAME.to_owned(),
                metadata: String::default(),
                metadata_signature: String::default(),
            },
            fetched: Fetched {
                order: FetchedOrder {
                    policy: Policy {
                        name: POLICY_NAME.to_owned(),
                        approvals: vec![Approval {
                            level: String::default(),
                            name: APPROVER_NAME.to_owned(),
                            response: Some(ApprovalResponse {
                                order_id: String::default(),
                                status_reason: String::default(),
                                approval_status: 0,
                                approver_name: APPROVER_NAME.to_owned(),
                                metadata: String::default(),
                                metadata_signature: String::default(),
                            }),
                        }],
                    },
                },
            },
        };

        UpdateOrderApproverResponse::run(request, &())
            .await
            .expect("should succeed and overwrite the response");
    }

    #[tokio::test]
    async fn approver_name_not_expected() {
        let invalid_approver_name = format!("{APPROVER_NAME}-fake");
        let request = ProcessApproverRequest {
            approval_response: ApprovalResponse {
                order_id: String::default(),
                status_reason: String::default(),
                approval_status: 0,
                approver_name: invalid_approver_name.clone(),
                metadata: String::default(),
                metadata_signature: String::default(),
            },
            fetched: Fetched {
                order: FetchedOrder {
                    policy: Policy {
                        name: POLICY_NAME.to_owned(),
                        approvals: vec![Approval {
                            level: String::default(),
                            name: APPROVER_NAME.to_owned(),
                            response: None,
                        }],
                    },
                },
            },
        };

        let error = UpdateOrderApproverResponse::run(request, &())
            .await
            .unwrap_err();
        assert!(matches!(error, OrchestrationError::Unknown(_)));
        assert!(error.to_string().contains(
            format!(
                "Order does not expect a response from approver named: {invalid_approver_name}"
            )
            .as_str()
        ));
    }

    #[tokio::test]
    async fn update_policy_successfully() {
        let order_id = Uuid::new_v4();
        let policy_name = "DefaultTenantSoloApproval";
        let approver_name = "forte_waas_txn_approver";
        let approval_status = 1;
        let level = "Tenant";
        let status_reason = "This is an auto-approved transaction";
        let metadata = "eyJhcHByb3ZhbF9zdGF0dXMiOjEsIm9yZGVyX2lkIjoiNDQ1NTI5YmYtNDVjNS00NzgwLThiNWUtOTgxOWMxNDIxMjUwIiwic3RhdHVzX3JlYXNvbiI6IlRoaXMgaXMgYW4gYXV0by1hcHByb3ZlZCB0cmFuc2FjdGlvbiIsInRyYW5zYWN0aW9uX2hhc2giOlsxMDgsMzcsMzcsMjA5LDYwLDIzMywxNDIsMjI4LDE0MiwxNTQsNzIsMTc0LDExMSwxOTYsNDYsNTksMTQ2LDI0MSwyNDQsMTA1LDQxLDQ0LDI1MSwwLDEwNiwxNzksMTk1LDIwMCwxMzEsMTMzLDMxLDIwMl19";
        let metadata_signature = "MEUCIQCXVE7ioy23HUKcWhoZAjemgCqeR9iZJ9ApciD7syre5AIgF3ehYtx0XQOb2b+DtLDuKF+qkheojQxxL6/HcCIaFjU=";

        let mut expected = ProcessApproverResponse {
            policy: Policy {
                name: policy_name.to_owned(),
                approvals: Vec::from([Approval {
                    level: level.to_owned(),
                    name: approver_name.to_owned(),
                    response: Some(ApprovalResponse {
                        order_id: order_id.to_string(),
                        status_reason: status_reason.to_owned(),
                        approval_status,
                        approver_name: approver_name.to_owned(),
                        metadata: metadata.to_owned(),
                        metadata_signature: metadata_signature.to_owned(),
                    }),
                }]),
            },
        };

        let request = ProcessApproverRequest {
            fetched: Fetched {
                order: FetchedOrder {
                    policy: Policy {
                        name: policy_name.to_owned(),
                        approvals: Vec::from([Approval {
                            level: level.to_owned(),
                            name: approver_name.to_owned(),
                            response: None,
                        }]),
                    },
                },
            },
            approval_response: ApprovalResponse {
                order_id: order_id.to_string(),
                status_reason: status_reason.to_owned(),
                approval_status,
                approver_name: approver_name.to_owned(),
                metadata: metadata.to_owned(),
                metadata_signature: metadata_signature.to_owned(),
            },
        };

        let mut result = UpdateOrderApproverResponse::run(request, &())
            .await
            .unwrap();

        // Make sure the response is actually there
        assert!(result.policy.approvals[0].response.is_some());

        // Verify response fields are set correctly
        assert_eq!(
            result.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .order_id,
            expected.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .order_id
        );
        assert_eq!(
            result.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .status_reason,
            expected.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .status_reason
        );
        assert_eq!(
            result.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .approval_status,
            expected.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .approval_status
        );
        assert_eq!(
            result.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .approver_name,
            expected.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .approver_name
        );
        assert_eq!(
            result.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .metadata,
            expected.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .metadata
        );
        assert_eq!(
            result.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .metadata_signature,
            expected.policy.approvals[0]
                .response
                .as_mut()
                .unwrap()
                .metadata_signature
        );
    }
}
