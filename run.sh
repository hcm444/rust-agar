#!/bin/bash

# Build the project
echo "Building the project..."
cargo build

# Run the server
echo "Starting the server at http://localhost:8080..."
RUST_LOG=info cargo run --bin multi 