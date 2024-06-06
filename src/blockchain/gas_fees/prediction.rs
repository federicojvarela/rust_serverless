use anyhow::{anyhow, Error};

use crate::blockchain::providers::EvmBlockchainProvider;

use super::{models::ProcessedFees, price_calculator::process_suggested_fees};

pub async fn get_predicted_fees(
    provider: &dyn EvmBlockchainProvider,
    chain_id: u64,
) -> Result<ProcessedFees, Error> {
    let fees = provider
        .get_fees_from_pending(chain_id)
        .await
        .map_err(|e| anyhow!(e).context("Error getting fees from pending blocks"))?;

    process_suggested_fees(&fees.max_priority_fees, fees.base_fee_per_gas)
        .map_err(|e| anyhow!(e).context("Error processing fees from pending blocks"))
}
