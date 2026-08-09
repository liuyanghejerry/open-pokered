#!/bin/bash
# Quick development server for pokered-web
# Builds and serves the WASM package locally

set -e

echo "Building pokered-web..."
./build-web.sh debug

echo ""
echo "Starting local server..."
echo "Open http://localhost:8080 in your browser"
echo ""
cd pkg
python3 -m http.server 8080