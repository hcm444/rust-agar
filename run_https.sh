#!/bin/bash

# Generate certificates if they don't exist
if [ ! -f "certs/cert.pem" ] || [ ! -f "certs/key.pem" ]; then
  echo "Certificates not found. Generating new certificates..."
  ./generate_cert.sh
fi

# Compile and run with HTTPS support
echo "Compiling with HTTPS support..."
cargo build --bin multi_https

# Check if the build was successful
if [ $? -eq 0 ]; then
  echo "Starting server with HTTPS on port 443..."
  # You need sudo for port 443
  sudo RUST_LOG=info ./target/debug/multi_https
else
  echo "Compilation failed!"
fi 