use crate::config::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;
use rstest::fixture;
use serde::Deserialize;
use std::sync::Arc;

type OrdersRepositoryObject = Arc<dyn OrdersRepository>;

#[derive(Deserialize)]
pub struct Config {
    pub order_status_table_name: String,
}
pub struct RepoFixture {
    pub orders_repository: OrdersRepositoryObject,
}

#[fixture]
#[once]
pub fn repo_fixture() -> RepoFixture {
    let config: Config = ConfigLoader::load_test();
    let order_table_name = config.order_status_table_name.clone();
    let dynamodb_client = get_dynamodb_client();
    let orders_repository = Arc::new(OrdersRepositoryImpl::new(order_table_name, dynamodb_client))
        as OrdersRepositoryObject;
    RepoFixture { orders_repository }
}
