#!/bin/bash

PRIVATE_KEY_PEM_FILE=$1
THING_TO_SIGN=$2

# Force remove all temp files in case something broke on a previous execution
rm -f data.sign
rm -f thing_to_sign
rm -f public_key.pub

echo "#### Signing: ${THING_TO_SIGN}"

# -n is required otherwise echo appends a line break to the end of the file, this will mess up signature verification
echo -n "$THING_TO_SIGN" >> thing_to_sign

# Signing algorithm is determined based on info encoded in the private key's PEM file (ASN.1 format). Should be ECDSA
# for everything we're doing. Payload will be hashed using keccak-256. Stick to keccak, some sha3-256 implementations
# are compatible with keccak, but support is all over the place. Openssl itself treats them separately.
#
# Raw output will be ASN.1 DER encoded. We'll manually base64 encode it after. Signatures are non-deterministic,
# openssl hasn't added support for deterministic signatures to their cli yet. This doesn't matter for verification.
# As long as it's on the same curve (correct public key) and the same input it'll verify the signature whether it's
# deterministic or not per the RFC6979 spec.
openssl dgst -sign "$PRIVATE_KEY_PEM_FILE" -keyform PEM -keccak-256 -out data.sign -binary thing_to_sign

# Derive public key from the private key (keeps the script simpler, don't have to explicitly pass in the public key)
PUBLIC_KEY=$(cat "$PRIVATE_KEY_PEM_FILE" | openssl pkey -pubout)
echo -n "$PUBLIC_KEY" >> public_key.pub

# Verify the signature
echo "#### Verifying..."
VERIFY_RESULT=$(openssl dgst -verify ./public_key.pub -keyform PEM -keccak-256 -signature data.sign -binary thing_to_sign)

if [ "$VERIFY_RESULT" != "Verified OK" ]; then
  echo "Failed to verify signature; Exiting."
  exit 1
fi

echo "Signature verified"

# base64 encode the ANS.1 DER signature
SIGNATURE=$(cat data.sign | base64 | tr -d '\n')
echo "#### Base64 Encoded Signature"

# This MUST be the last thing we echo out so we can parse it in other script files. -n omits new line character
echo -n "${SIGNATURE}"

# Clean up
rm data.sign
rm thing_to_sign
rm public_key.pub