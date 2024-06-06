use ethers::types::U256;

use super::models::{ProcessedFees, SuggestedFees, SuggestedFeesProcessingError};

fn percentile(prices: &[U256], p: f64) -> (U256, usize) {
    let len = prices.len() as f64;
    let index = (p * len).ceil() as usize - 1;
    let last_price = prices.last();

    let result = last_price
        .map(|price| *prices.get(index).unwrap_or(price))
        .unwrap_or(U256::from(0));
    (result, index)
}

pub fn process_suggested_fees(
    max_priority_fees: &[U256],
    base_fee_per_gas: U256,
) -> Result<ProcessedFees, SuggestedFeesProcessingError> {
    if max_priority_fees.is_empty() {
        return Err(SuggestedFeesProcessingError::ArrayIsEmpty);
    }

    let max_priority_fee_per_gas = SuggestedFees {
        low: percentile(max_priority_fees, 0.25).0,
        medium: percentile(max_priority_fees, 0.50).0,
        high: percentile(max_priority_fees, 0.95).0,
    };

    let max_fee_per_gas = SuggestedFees {
        low: max_priority_fee_per_gas.low + base_fee_per_gas,
        medium: max_priority_fee_per_gas.medium + base_fee_per_gas,
        high: max_priority_fee_per_gas.high + base_fee_per_gas,
    };

    Ok(ProcessedFees {
        max_priority_fee_per_gas,
        max_fee_per_gas: max_fee_per_gas.clone(),
        gas_price: max_fee_per_gas.clone(),
    })
}

#[cfg(test)]
mod tests {
    use ethers::types::U256;

    use crate::blockchain::gas_fees::{
        models::SuggestedFees, price_calculator::process_suggested_fees,
    };

    #[test]
    fn test_process_suggested_fees_success() {
        let max_priority_fee_per_gas = vec![U256::from(11), U256::from(12), U256::from(13)];
        let base_fee_per_gas = U256::from(100);

        let expected_max_priority_fee_per_gas = SuggestedFees {
            low: U256::from(11),
            medium: U256::from(12),
            high: U256::from(13),
        };
        let expected_max_fee_per_gas = SuggestedFees {
            low: U256::from(111),
            medium: U256::from(112),
            high: U256::from(113),
        };

        let result = process_suggested_fees(&max_priority_fee_per_gas, base_fee_per_gas).unwrap();

        assert_eq!(
            expected_max_priority_fee_per_gas,
            result.max_priority_fee_per_gas
        );
        assert_eq!(expected_max_fee_per_gas, result.max_fee_per_gas);
    }

    #[test]
    fn test_process_suggested_fees_max_priority_empty() {
        let max_priority_fee_per_gas = vec![];
        let base_fee_per_gas = U256::from(100);

        process_suggested_fees(&max_priority_fee_per_gas, base_fee_per_gas).unwrap_err();
    }
}
