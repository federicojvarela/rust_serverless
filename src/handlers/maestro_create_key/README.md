# Maestro Create Key

## Environment

- `AWS_REGION`: Value is `us-west-2`.
- `MAESTRO_URL`: Value is `https://maestro.staging.boltlabs.io`.
- `SERVICE_NAME`: Value is `service_provider`.
- `MAESTRO_API_KEY_SECRET_NAME`: Value of AWS secret of local seeded secret.

## Payload

```
cargo lambda invoke maestro_create_key --data-ascii '{ "payload": {}, "context": { "order_id": "1b445c3e-423a-4b74-ba3d-554dcd414efc" } } '
```
