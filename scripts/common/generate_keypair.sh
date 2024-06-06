#!/bin/bash

# Generates a brand new keypair using secp256k1 elliptic curve. Outputs result as PEM encoded text to the console as
# well as plain text versions for import in a 3rd party wallet

PEM_FILE="keypair.pem"

# Force remove .pem file in case it exists (something broke on a previous run)
rm -f $PEM_FILE

openssl ecparam -name secp256k1 -genkey -noout -out $PEM_FILE

PRIVATE_KEY=$(cat "$PEM_FILE" | openssl pkey)
PUBLIC_KEY=$(cat "$PEM_FILE" | openssl pkey -pubout)

TEXT_WHOLE_KEY=$(cat "$PEM_FILE" | openssl ec -text -noout)
TEXT_PUBLIC_KEY=$(echo "$TEXT_WHOLE_KEY" | grep pub -A 5 | tail -n +2 | tr -d '\n[:space:]:' | sed 's/^04//')
TEXT_PRIVATE_KEY=$(echo "$TEXT_WHOLE_KEY" | grep priv -A 3 | tail -n +2 | tr -d '\n[:space:]:' | sed 's/^00//')

printf '###### Private Key PEM (PKCS8)######\n%s\n\n' "$PRIVATE_KEY"
printf '###### Public Key PEM (PKCS8)######\n%s\n\n' "$PUBLIC_KEY"
printf '###### Public Key Text (keccak256 this for public eth address) ######\n%s\n\n' "$TEXT_PUBLIC_KEY"
printf '###### Private Key Text (can import this into a 3rd party wallet e.g. metamask) ######\n%s\n\n' "$TEXT_PRIVATE_KEY"

rm $PEM_FILE