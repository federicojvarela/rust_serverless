// TODO: Remove this when this is used in the lambda
#![allow(unused)]
use crate::models::HistoricalFees;
use ethers::types::U256;
use mpc_signature_sm::blockchain::providers::FeeHistory;
use serde::{self, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct ProcessedHistoricalFees {
    pub max_priority_fee_per_gas: HistoricalFees,
    pub max_fee_per_gas: HistoricalFees,
    pub gas_price: HistoricalFees,
}

#[derive(Debug, thiserror::Error)]
pub enum FeeHistoryProcessingError {
    #[error("the \"{0}\" array was empty, check the RPC response")]
    ArrayIsEmpty(&'static str),
}

/// Calculates the median value of an Array. Panics if the array is empty!
fn calculate_median(values: &mut [U256]) -> U256 {
    values.sort();

    let values_length = values.len();

    if values_length % 2 == 0 {
        (values[values_length / 2] + values[values_length / 2 - 1]) / 2
    } else {
        values[values_length / 2]
    }
}

pub fn get_historical_fees(
    mut fee_history: FeeHistory,
) -> Result<ProcessedHistoricalFees, FeeHistoryProcessingError> {
    // This follows the steps outlined here: https://github.com/fortelabsinc/mpc-signature-sm/blob/main/src/handlers/mpc_get_historical_fees/README.md
    // Part 1
    let max_priority_fee_per_gas = get_max_priority_fee_per_gas(&fee_history)?;
    let max_fee_per_gas = get_max_fee_per_gas(&mut fee_history, &max_priority_fee_per_gas)?;
    let gas_price = max_fee_per_gas.clone();

    Ok(ProcessedHistoricalFees {
        max_priority_fee_per_gas,
        max_fee_per_gas,
        gas_price,
    })
}

/// This method implements [Part 1 of the algorithm](https://github.com/fortelabsinc/mpc-signature-sm/blob/main/src/handlers/mpc_get_historical_fees/README.md)
/// that calculates the response.
fn get_max_priority_fee_per_gas(
    fee_history: &FeeHistory,
) -> Result<HistoricalFees, FeeHistoryProcessingError> {
    let reward_length = fee_history.reward.len();

    if fee_history.reward.is_empty() {
        return Err(FeeHistoryProcessingError::ArrayIsEmpty("reward"));
    }

    let mut min_vals = vec![];
    let mut max_vals = vec![];
    let mut median_vals = vec![];

    for row in &fee_history.reward {
        if row.is_empty() {
            return Err(FeeHistoryProcessingError::ArrayIsEmpty("reward detail"));
        }

        min_vals.push(row[0]);
        median_vals.push(row[row.len() / 2]);
        max_vals.push(row[row.len() - 1]);
    }

    let median = calculate_median(&mut median_vals);
    min_vals.sort();
    let min = min_vals[0];
    max_vals.sort();
    let max = max_vals[max_vals.len() - 1];

    Ok(HistoricalFees { min, max, median })
}

/// This method implements [Part 2 of the algorithm](https://github.com/fortelabsinc/mpc-signature-sm/blob/main/src/handlers/mpc_get_historical_fees/README.md)
/// that calculates the response.
fn get_max_fee_per_gas(
    fee_history: &mut FeeHistory,
    max_priority_fee_per_gas: &HistoricalFees,
) -> Result<HistoricalFees, FeeHistoryProcessingError> {
    let base_fee_per_gas_length = fee_history.base_fee_per_gas.len();

    if base_fee_per_gas_length == 0 {
        return Err(FeeHistoryProcessingError::ArrayIsEmpty("base_fee_per_gas"));
    }

    let median_fee = calculate_median(&mut fee_history.base_fee_per_gas);

    Ok(HistoricalFees {
        min: median_fee + max_priority_fee_per_gas.min,
        max: median_fee + max_priority_fee_per_gas.max,
        median: median_fee + max_priority_fee_per_gas.median,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        fees_calculator::{get_historical_fees, FeeHistoryProcessingError},
        models::HistoricalFees,
    };
    use ethers::types::U256;
    use mpc_signature_sm::blockchain::providers::FeeHistory;

    fn create_fee_history_input(reward: Vec<Vec<U256>>, base_fee_per_gas: Vec<U256>) -> FeeHistory {
        FeeHistory {
            oldest_block: reward.len() as u64,
            reward,
            base_fee_per_gas,
            gas_used_ratio: vec![],
        }
    }

    #[test]
    fn test_fee_history_num_of_blocks_1() {
        let reward = vec![vec![U256::from(11), U256::from(12), U256::from(13)]];
        let base_fee_per_gas = vec![U256::from(100)];
        let input = create_fee_history_input(reward, base_fee_per_gas);
        let result = get_historical_fees(input).unwrap();

        let expected_max_priority_fee_per_gas = HistoricalFees {
            min: U256::from(11),
            max: U256::from(13),
            median: U256::from(12),
        };
        let expected_max_fee_per_gas = HistoricalFees {
            min: U256::from(111),
            max: U256::from(113),
            median: U256::from(112),
        };

        assert_eq!(
            expected_max_priority_fee_per_gas,
            result.max_priority_fee_per_gas
        );
        assert_eq!(expected_max_fee_per_gas, result.max_fee_per_gas);
    }

    #[test]
    fn test_fee_history_num_of_blocks_3() {
        let reward = vec![
            vec![U256::from(31), U256::from(32), U256::from(33)],
            vec![U256::from(11), U256::from(12), U256::from(13)],
            vec![U256::from(21), U256::from(22), U256::from(23)],
        ];
        let base_fee_per_gas = vec![U256::from(200), U256::from(300), U256::from(100)];
        let input = create_fee_history_input(reward, base_fee_per_gas);

        let result = get_historical_fees(input).unwrap();

        let expected_max_priority_fee_per_gas = HistoricalFees {
            min: U256::from(11),
            max: U256::from(33),
            median: U256::from(22),
        };
        let expected_max_fee_per_gas = HistoricalFees {
            min: U256::from(211),
            max: U256::from(233),
            median: U256::from(222),
        };

        assert_eq!(
            expected_max_priority_fee_per_gas,
            result.max_priority_fee_per_gas
        );
        assert_eq!(expected_max_fee_per_gas, result.max_fee_per_gas);
    }

    #[test]
    fn test_fee_base_fee_per_gas_is_empty() {
        let reward = vec![
            vec![U256::from(11), U256::from(12), U256::from(13)],
            vec![U256::from(21), U256::from(22), U256::from(33)],
        ];
        let base_fee_per_gas = vec![];
        let input = create_fee_history_input(reward, base_fee_per_gas);

        let result = get_historical_fees(input).unwrap_err();

        assert!(matches!(
            result,
            FeeHistoryProcessingError::ArrayIsEmpty("base_fee_per_gas"),
        ));
    }
}
