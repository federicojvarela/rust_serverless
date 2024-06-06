# Maestro Sign Request

This lambda is used to `Maestro` service to sign requests.

## Running

In order to run this lambda locally, some environment variables must be set:

- `MAESTRO_URL`: Maestro's URL.
- `SERVICE_NAME`: Maestro's service name used in the `/sign` call.
- `AWS_REGION`: **NOTE**: Define this only for dev purposes! AWS Already provides this env variable in lambda. AWS region for where the Secret Manager is located.
- `MAESTRO_API_KEY_SECRET_NAME`: Name of secret that contains the API key needed for logging in with Maestro.
- `LOCALSTACK_TEST_MODE_ENDPOINT`: Only for development. The localstack endpoint. Default value: `None`.
- `AWS_ACCESS_KEY_ID`: Only for development. Needed by the `secrets_provider` crate to communicate with localstack. You can put any value here.
- `AWS_SECRET_ACCESS_KEY`: Only for development. Needed by the `secrets_provider` crate to communicate with localstack. You can put any value here.

### Local stack

To be able to use the lambda locally, the Secrets Manager service must be up and running. Execute the following command to run it:

```bash
SECRET_MAESTRO_API_KEY=<MAESTRO_API_KEY> docker-compose up
```

The secret manager will be seeded with the Maestro's api secret. The secret key is `maestro-api-secret`.

### Cargo lambda watch

In order to correctly invoke the lambda, the `cargo lambda watch` command must be executed with all the environment variables set:

```bash
MAESTRO_URL=https://maestro.dev.boltlabs.io \
SERVICE_NAME=test \
AWS_REGION=us-west-2 \
LOCALSTACK_TEST_MODE_ENDPOINT=http://127.0.0.1:4566 \
MAESTRO_API_KEY_SECRET_NAME=maestro-api-secret \
AWS_ACCESS_KEY_ID=test \
AWS_SECRET_ACCESS_KEY=test \
cargo lambda watch
```

### Cargo lambda invoke

Here's an invocation example that should return a successful answer:

```bash
cargo lambda invoke maestro_sign_request --data-ascii '{"payload": {"transaction": { "to": "0x1c965d1241D0040A3fC2A030BaeeEfB35C155a4e", "gas": "300000", "gas_price": "300000000", "value": "111111", "nonce": "15", "data": "0x6406516041610651325106165165106516169610" }, "transaction_type": "Unknown", "transaction_hash": "3ac225168df54212a25c1c01fd35bebfea408fdac2e31ddd6f80a4bbf9a5f1cb", "key_id": "53", "entity_signature_authorizing": "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d", "entity_authorizing_name": "test" }, "context": { "order_id": "1b445c3e-423a-4b74-ba3d-554dcd414efc" } }'
```

## Testing

To test run:

```bash
cargo test
``
```
