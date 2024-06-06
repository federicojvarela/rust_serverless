use async_trait::async_trait;
use ethers::types::Address;
use uuid::Uuid;

pub mod authorization_provider_by_address;
pub mod authorization_provider_by_order;
pub mod errors;

pub use authorization_provider_by_address::AuthorizationProviderByAddressImpl;
pub use authorization_provider_by_order::AuthorizationProviderByOrderImpl;
pub use errors::AuthorizationProviderError;

#[async_trait]
pub trait AuthorizationProviderByAddress {
    async fn client_id_has_address_permission(
        &self,
        address: Address,
        client_id: &str,
    ) -> Result<bool, AuthorizationProviderError>;
}

#[async_trait]
pub trait AuthorizationProviderByOrder {
    async fn client_id_has_order_permission(
        &self,
        order_id: Uuid,
        client_id: &str,
    ) -> Result<bool, AuthorizationProviderError>;
}
