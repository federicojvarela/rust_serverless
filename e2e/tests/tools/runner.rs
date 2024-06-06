use crate::tools::fixtures::e2e::E2EFixture;
use crate::tools::fixtures::e2e::TestContext;
use crate::tools::helper::{
    print_test_output_line_begins, print_test_output_line_ends, print_with_time,
};
use std::future::Future;

pub async fn run_e2e_test<F, Fut>(test_name: &str, f: F, fixture: &E2EFixture)
where
    F: Fn(TestContext) -> Fut,
    Fut: Future<Output = ()>,
{
    print_test_output_line_begins();
    let config = fixture.config.clone();
    let chain = &config.chain;
    let total_networks = chain.chain_network.len();

    for (current_network_index, network) in chain.chain_network.iter().enumerate() {
        let test_context = TestContext {
            config: config.clone(),
            chain_id: network.id,
            client: fixture.client.clone(),
        };

        print_with_time(format!(
            "[+] Starting Rust E2E {} on {} Environment, Chain: {}, Network {}/{}: {}, ID: {}",
            test_name,
            config.environment,
            chain.name,
            current_network_index + 1,
            total_networks,
            network.name,
            network.id
        ));

        f(test_context.clone()).await;

        print_with_time(format!(
            "[-] Finished Rust E2E {} on {} Environment, Chain: {}, Network {}/{}: {}, ID: {}",
            test_name,
            config.environment,
            chain.name,
            current_network_index + 1,
            total_networks,
            network.name,
            network.id
        ));
    }
    print_test_output_line_ends();
}
