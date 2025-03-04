#!/bin/bash
set -euo pipefail

cargo build --release
cp ./target/release/playlister ./arch-pkg/
