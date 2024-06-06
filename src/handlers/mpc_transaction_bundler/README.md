# `mpc_transaction_bundler` lambda

This lambda sends regular transactions from a "gas pool" address, which means that there's a need for a nonce table entry for this gas pool in every enviroment. Currently, it only exists in dev and **hasn't been setup for ephemeral environments yet**. If you have the need to test this lambda on those,
you need to create a new address using our WaaS, fund it, and set it as the value of the `GAS_POOL_ADDRESS` environment variable.
