use common::aws_clients::sqs::get_sqs_client;
use rstest::fixture;
use rusoto_sqs::SqsClient;

pub struct SqsFixture {
    pub sqs_client: SqsClient,
}

#[fixture]
#[once]
pub fn sqs_fixture() -> SqsFixture {
    SqsFixture {
        sqs_client: get_sqs_client(),
    }
}
