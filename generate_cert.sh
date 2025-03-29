#!/bin/bash

# Create certs directory if it doesn't exist
mkdir -p certs

# Generate a private key
openssl genrsa -out certs/key.pem 2048

# Generate a certificate signing request
openssl req -new -key certs/key.pem -out certs/csr.pem -subj "/CN=localhost"

# Generate a self-signed certificate
openssl x509 -req -days 365 -in certs/csr.pem -signkey certs/key.pem -out certs/cert.pem

# Display success message
echo "Self-signed certificate generated successfully!"
echo "Certificate: certs/cert.pem"
echo "Private key: certs/key.pem" 