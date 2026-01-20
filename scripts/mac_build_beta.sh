#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CSV_FILE="$PROJECT_ROOT/beta_testers.csv"
OUTPUT_DIR="$PROJECT_ROOT/beta_builds"
DMG_SOURCE="$PROJECT_ROOT/src-tauri/target/universal-apple-darwin/release/bundle/dmg/Oto Desktop_0.1.0_universal.dmg"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Load Apple code signing credentials
source "$PROJECT_ROOT/.env.local"
export APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID

# Skip header line and process each user
tail -n +2 "$CSV_FILE" | while IFS=',' read -r discord_id username api_key; do
    echo "=========================================="
    echo "Building for: $username"
    echo "=========================================="

    # Build with hardcoded API key and code signing
    (cd "$PROJECT_ROOT" && OPENAI_API_KEY="$api_key" bun run build:mac)

    # Create user folder and copy the DMG
    mkdir -p "$OUTPUT_DIR/$username"
    cp "$DMG_SOURCE" "$OUTPUT_DIR/$username/"

    echo "Created: $OUTPUT_DIR/$username/Oto Desktop_0.1.0_universal.dmg"
done

echo "=========================================="
echo "All builds complete!"
echo "Output directory: $OUTPUT_DIR"
ls -la "$OUTPUT_DIR"
