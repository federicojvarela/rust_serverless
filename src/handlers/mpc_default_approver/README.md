# Default Approver

This lambda is used as a transaction approver. Client can choose to have their own approver or fallback to this one (that approves all transactions).
More than one event can be sent to be processed. This lambda process all the events concurrently.


## Running

In order to run this lambda locally, some environment variables must be set:

- `AWS_REGION`: **NOTE**: Define this only for dev purposes! AWS Already provides this env variable in lambda.
- `RESPONSE_QUEUE_URL`: Queue URL where approver will send the response.
- `LOCALSTACK_TEST_MODE_ENDPOINT`: Only for development. The localstack endpoint. Default value: `None`.
- `AWS_ACCESS_KEY_ID`: Only for development. Needed by the `rusoto` crate to communicate with localstack. You can put any value here.
- `AWS_SECRET_ACCESS_KEY`: Only for development. Needed by the `rusoto` crate to communicate with localstack. You can put any value here.

### Local stack

To be able to use the lambda locally, the SQS emulator must be up and running. Execute the following command to run it:

```bash
docker-compose up
```

The SQS will be create the approver response queue at the beggining.

### Cargo lambda watch

In order to correctly invoke the lambda, the `cargo lambda watch` command must be executed with all the environment variables set:

```bash
AWS_REGION=us-west-2 \
RESPONSE_QUEUE_URL=http://localstack:4566/000000000000/compliance-response \
AWS_ACCESS_KEY_ID=test \
LOCALSTACK_TEST_MODE_ENDPOINT=http://127.0.0.1:4566 \
AWS_SECRET_ACCESS_KEY=test \
cargo lambda watch
```

### Cargo lambda invoke

Here's an invocation example that should send to the queue an accepted order:

```bash
cargo lambda invoke mpc_default_approver --data-ascii '[{"transaction": { "to": "0x1c965d1241D0040A3fC2A030BaeeEfB35C155a42", "gas": "300000", "gas_price": "300000000", "value": "111111", "nonce": "0", "data": "0x6406516041610651325106165165106516169610" }, "contextual_data": { "order_id": "1b445c3e-423a-4b74-ba3d-554dcd414efc" } }]'
```

### Fetching the results

In order to fetch the results you will need to have the AWS CLI tool installed and run:
```bash
aws --endpoint-url=http://localhost:4566/_aws/sqs/messages sqs receive-message --queue-url=http://localstack:4566/000000000000/compliance-response --region us-west-2
```
